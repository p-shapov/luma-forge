use crate::{
    domain::workspace::{provisioning_state::fail_workspace, WorkspaceLifecycleState},
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
    workspace_provisioner::WorkspaceProvisionerService,
    workspace_resources::WorkspaceResourceService,
};

use super::{
    context::{WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    coordinator::WorkspaceProvisioningCoordinator,
    failure,
    helpers::{result, WorkspaceProvisioningResult},
    steps, WorkspaceProvisioningError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceProvisioningConfig {
    pub volume_mount_path: String,
}

pub struct WorkspaceProvisioningService<S, W, R, Q = WorkspaceResourceService<S, W>> {
    secrets: S,
    resources: Q,
    workspace_catalog: W,
    workers: R,
    workspace_provisioner: WorkspaceProvisionerService,
    coordinator: WorkspaceProvisioningCoordinator,
    config: WorkspaceProvisioningConfig,
}

impl<S, W, R, Q> WorkspaceProvisioningService<S, W, R, Q> {
    pub fn new(
        secrets: S,
        resources: Q,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        config: WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            workspace_provisioner: WorkspaceProvisionerService::new(),
            coordinator,
            config,
        }
    }

    fn context(&self) -> WorkspaceProvisioningContext<'_, S, W, R, Q> {
        WorkspaceProvisioningContext::new(
            &self.secrets,
            &self.resources,
            &self.workspace_catalog,
            &self.workers,
            &self.workspace_provisioner,
            &self.config,
        )
    }
}

impl<S, W, R, Q> WorkspaceProvisioningService<S, W, R, Q>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    pub async fn initiate(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Draft {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }
        self.secrets
            .read_api_key(&workspace.gpu_cloud_provider_id)
            .map_err(WorkspaceProvisioningError::from)?
            .ok_or(WorkspaceProvisioningError::ProviderSetupIncomplete)?;

        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.last_provisioning_failure = None;
        let workspace = context.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }

    pub async fn sync(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            let context = self.context();
            return Ok(result(context.workspace(workspace_id).await?));
        };

        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Ok(result(workspace));
        }

        if let Some(result) = steps::sync(&context, &mut workspace).await? {
            return Ok(result);
        }

        Ok(result(workspace))
    }

    pub async fn cancel(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Err(WorkspaceProvisioningError::ProviderOperationConflict);
        };

        let context = self.context();
        let mut workspace = context.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }

        match self.resources.cleanup_known_resources(&mut workspace).await {
            Ok(updated_workspace) => {
                workspace = updated_workspace;
            }
            Err(_) => {
                fail_workspace(&mut workspace, failure::cancellation_cleanup_failed());
                workspace = context.update_workspace(&workspace).await?;
            }
        }

        Ok(result(workspace))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            provider_setup::GpuCloudProviderId,
            workspace::{
                ProviderResourceStatus, WorkspaceLifecycleState, WorkspaceProvisioningFailureCode,
                WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
                WorkspaceProvisioningRecoveryAction, WorkspaceProvisioningStatus,
            },
        },
        secrets::SecretStoreError,
        workspace_resources::WorkspaceResourceError,
    };

    use super::*;
    use crate::workspace_provisioning::test_support::{
        pod, provisioning_workspace, service_parts, volume, workspace,
        FakeProvisionerWorkerGateway, FakeSecretStore, FakeWorkspaceCatalog,
        FakeWorkspaceResources,
    };

    #[tokio::test]
    async fn initiate_transitions_draft_to_provisioning_without_resource_mutation() {
        let mut workspace = workspace();
        workspace.last_provisioning_failure = Some(failure::cancellation_cleanup_failed());
        let (service, secrets, catalog, resources, _, _) = service_parts(workspace);

        let result = service
            .initiate("workspace-1")
            .await
            .expect("initiate should succeed");

        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        assert_eq!(result.progress.status, WorkspaceProvisioningStatus::Running);
        assert!(result.workspace.last_provisioning_failure.is_none());
        assert_eq!(
            secrets.read_api_key_calls(),
            vec![GpuCloudProviderId::Runpod]
        );
        assert!(resources.calls().is_empty());
        assert_eq!(catalog.updates().len(), 1);

        let serialized = serde_json::to_string(&result.workspace).expect("serialize workspace");
        assert!(!serialized.contains("provider-secret"));
    }

    #[tokio::test]
    async fn initiate_rejects_non_draft_without_mutation() {
        let (service, secrets, catalog, resources, _, _) = service_parts(provisioning_workspace());

        let error = service
            .initiate("workspace-1")
            .await
            .expect_err("initiate should reject non-draft");

        assert_eq!(error, WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        assert!(secrets.read_api_key_calls().is_empty());
        assert!(catalog.updates().is_empty());
        assert!(resources.calls().is_empty());
    }

    #[tokio::test]
    async fn initiate_rejects_provider_key_failures_before_lifecycle_transition() {
        for (api_key_result, expected_error) in [
            (
                Ok(None),
                WorkspaceProvisioningError::ProviderSetupIncomplete,
            ),
            (
                Err(SecretStoreError::InvalidStoredProviderApiKey),
                WorkspaceProvisioningError::ProviderSetupIncomplete,
            ),
            (
                Err(SecretStoreError::SecureKeyringUnavailable),
                WorkspaceProvisioningError::SecureKeyringUnavailable,
            ),
        ] {
            let secrets = FakeSecretStore::with_api_key_result(api_key_result);
            let catalog = FakeWorkspaceCatalog::with_workspace(workspace());
            let resources = FakeWorkspaceResources::default();
            let service = WorkspaceProvisioningService::new(
                secrets,
                resources.clone(),
                catalog.clone(),
                FakeProvisionerWorkerGateway::default(),
                WorkspaceProvisioningCoordinator::default(),
                WorkspaceProvisioningConfig {
                    volume_mount_path: "/workspace".to_string(),
                },
            );

            let error = service
                .initiate("workspace-1")
                .await
                .expect_err("initiate should reject provider key failure");

            assert_eq!(error, expected_error);
            assert!(catalog.updates().is_empty());
            assert!(resources.calls().is_empty());
            assert_eq!(
                catalog
                    .stored_workspace()
                    .expect("workspace should remain stored")
                    .lifecycle_state,
                WorkspaceLifecycleState::Draft
            );
        }
    }

    #[tokio::test]
    async fn initiate_maps_missing_workspace_and_catalog_failure() {
        let missing_service = WorkspaceProvisioningService::new(
            FakeSecretStore::with_api_key("provider-secret"),
            FakeWorkspaceResources::default(),
            FakeWorkspaceCatalog::missing(),
            FakeProvisionerWorkerGateway::default(),
            WorkspaceProvisioningCoordinator::default(),
            WorkspaceProvisioningConfig {
                volume_mount_path: "/workspace".to_string(),
            },
        );
        assert_eq!(
            missing_service
                .initiate("workspace-1")
                .await
                .expect_err("missing workspace should fail"),
            WorkspaceProvisioningError::WorkspaceNotFound
        );

        let unavailable_service = WorkspaceProvisioningService::new(
            FakeSecretStore::with_api_key("provider-secret"),
            FakeWorkspaceResources::default(),
            FakeWorkspaceCatalog::unavailable(),
            FakeProvisionerWorkerGateway::default(),
            WorkspaceProvisioningCoordinator::default(),
            WorkspaceProvisioningConfig {
                volume_mount_path: "/workspace".to_string(),
            },
        );
        assert_eq!(
            unavailable_service
                .initiate("workspace-1")
                .await
                .expect_err("catalog failure should fail"),
            WorkspaceProvisioningError::WorkspaceCatalogUnavailable
        );
    }

    #[tokio::test]
    async fn sync_non_provisioning_workspace_is_read_only() {
        for lifecycle_state in [
            WorkspaceLifecycleState::Draft,
            WorkspaceLifecycleState::Ready,
            WorkspaceLifecycleState::Failed,
        ] {
            let mut workspace = workspace();
            workspace.lifecycle_state = lifecycle_state.clone();
            let (service, _, catalog, resources, workers, _) = service_parts(workspace);

            let result = service
                .sync("workspace-1")
                .await
                .expect("sync should return current workspace");

            assert_eq!(result.workspace.lifecycle_state, lifecycle_state);
            assert!(catalog.updates().is_empty());
            assert!(resources.calls().is_empty());
            assert!(workers.status_calls().is_empty());
            assert!(workers.start_calls().is_empty());
        }
    }

    #[tokio::test]
    async fn concurrent_sync_is_read_only() {
        let (service, _, catalog, resources, workers, coordinator) =
            service_parts(provisioning_workspace());
        let _guard = coordinator
            .try_enter("workspace-1")
            .expect("test should acquire coordinator lock");

        let result = service
            .sync("workspace-1")
            .await
            .expect("concurrent sync should return persisted workspace");

        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        assert!(catalog.updates().is_empty());
        assert!(resources.calls().is_empty());
        assert!(workers.status_calls().is_empty());
    }

    #[tokio::test]
    async fn sync_provider_retryable_errors_do_not_persist_failure() {
        for resource_error in [
            WorkspaceResourceError::ProviderRateLimited,
            WorkspaceResourceError::ProviderOperationConflict,
            WorkspaceResourceError::ProviderRequestRejected,
        ] {
            let (service, _, catalog, resources, _, _) = service_parts(provisioning_workspace());
            resources.push_network_volume_result(Err(resource_error.clone()));

            let error = service
                .sync("workspace-1")
                .await
                .expect_err("sync should return provider command error");

            assert_eq!(error, WorkspaceProvisioningError::from(resource_error));
            assert!(catalog.updates().is_empty());
            assert_eq!(
                catalog
                    .stored_workspace()
                    .expect("workspace should remain stored")
                    .lifecycle_state,
                WorkspaceLifecycleState::Provisioning
            );
        }
    }

    #[tokio::test]
    async fn cancel_success_returns_cleanup_policy_result() {
        let (service, _, catalog, resources, _, _) = service_parts(provisioning_workspace());
        let clean_workspace = workspace();
        resources.push_cleanup_result(Ok(clean_workspace.clone()));

        let result = service
            .cancel("workspace-1")
            .await
            .expect("cancel should succeed");

        assert_eq!(resources.calls(), vec!["cleanup"]);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Draft
        );
        assert!(result
            .workspace
            .persistent_storage_volume_snapshot
            .is_none());
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn cancel_cleanup_failure_marks_failed_and_preserves_metadata() {
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        let (service, _, catalog, resources, _, _) = service_parts(workspace);
        resources.push_cleanup_result(Err(WorkspaceResourceError::ProviderApiUnavailable));

        let result = service
            .cancel("workspace-1")
            .await
            .expect("cancel should return failed workspace");

        assert_eq!(resources.calls(), vec!["cleanup"]);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        assert!(result
            .workspace
            .persistent_storage_volume_snapshot
            .is_some());
        assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
        let failure = result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted");
        assert_eq!(
            failure.code,
            WorkspaceProvisioningFailureCode::CancellationCleanupFailed
        );
        assert_eq!(failure.phase, WorkspaceProvisioningPhase::CleaningUp);
        assert_eq!(failure.source, WorkspaceProvisioningFailureSource::Native);
        assert_eq!(
            failure.recovery_action,
            WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources
        );
        assert_eq!(catalog.updates().len(), 1);
    }

    #[tokio::test]
    async fn cancel_conflict_performs_no_cleanup() {
        let (service, _, catalog, resources, _, coordinator) =
            service_parts(provisioning_workspace());
        let _guard = coordinator
            .try_enter("workspace-1")
            .expect("test should acquire coordinator lock");

        let error = service
            .cancel("workspace-1")
            .await
            .expect_err("cancel should conflict");

        assert_eq!(error, WorkspaceProvisioningError::ProviderOperationConflict);
        assert!(resources.calls().is_empty());
        assert!(catalog.updates().is_empty());
    }
}
