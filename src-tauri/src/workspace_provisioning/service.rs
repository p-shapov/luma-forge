use crate::{
    domain::workspace::WorkspaceLifecycleState,
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::WorkspaceResourceService,
};

use super::{
    context::{WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    coordinator::WorkspaceProvisioningCoordinator,
    failure::{self, fail_workspace},
    gateway::ProvisionerWorkerGateway,
    helpers::{progress_for_workspace, result, WorkspaceProvisioningResult},
    providers::{WorkspaceProvisioningProviderRegistry, WorkspaceProvisioningProviderResolver},
    provisioner::WorkspaceProvisionerService,
    WorkspaceProvisioningError,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceProvisioningConfig;

pub struct WorkspaceProvisioningService<
    S,
    W,
    R,
    Q = WorkspaceResourceService<S, W>,
    P = WorkspaceProvisioningProviderRegistry,
> {
    secrets: S,
    resources: Q,
    workspace_catalog: W,
    workers: R,
    provider_registry: P,
    workspace_provisioner: WorkspaceProvisionerService,
    coordinator: WorkspaceProvisioningCoordinator,
}

impl<S, W, R, Q> WorkspaceProvisioningService<S, W, R, Q, WorkspaceProvisioningProviderRegistry> {
    pub fn new(
        secrets: S,
        resources: Q,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        _config: WorkspaceProvisioningConfig,
    ) -> Self {
        Self::with_provider_registry(
            secrets,
            resources,
            workspace_catalog,
            workers,
            coordinator,
            _config,
            WorkspaceProvisioningProviderRegistry::default(),
        )
    }
}

impl<S, W, R, Q, P> WorkspaceProvisioningService<S, W, R, Q, P> {
    pub(crate) fn with_provider_registry(
        secrets: S,
        resources: Q,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        _config: WorkspaceProvisioningConfig,
        provider_registry: P,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            provider_registry,
            workspace_provisioner: WorkspaceProvisionerService::new(),
            coordinator,
        }
    }

    fn context(&self) -> WorkspaceProvisioningContext<'_, S, W, R, Q> {
        WorkspaceProvisioningContext::new(
            &self.secrets,
            &self.resources,
            &self.workspace_catalog,
            &self.workers,
            &self.workspace_provisioner,
        )
    }
}

impl<S, W, R, Q, P> WorkspaceProvisioningService<S, W, R, Q, P>
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
    P: WorkspaceProvisioningProviderResolver<S, W, R, Q>,
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
            .await
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

        let sync_result = self
            .provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .sync(&context, &mut workspace)
            .await;

        match sync_result {
            Ok(Some(result)) => return Ok(result),
            Ok(None) => {}
            Err(error) => {
                if let Some(failure) =
                    failure::provisioning_error(progress_for_workspace(&workspace).phase, &error)
                {
                    fail_workspace(&mut workspace, failure);
                    let workspace = context.update_workspace(&workspace).await?;
                    return Ok(result(workspace));
                }
                return Err(error);
            }
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

        match self
            .provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .cancel(&context, &mut workspace)
            .await
        {
            Ok(updated_workspace) => {
                workspace = updated_workspace;
                workspace.lifecycle_state = WorkspaceLifecycleState::Draft;
                workspace.last_provisioning_failure = None;
                workspace = context.update_workspace(&workspace).await?;
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
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use crate::{
        domain::{
            provider_setup::GpuCloudProviderId,
            workspace::{
                ProviderResourceStatus, Workspace, WorkspaceLifecycleState,
                WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
                WorkspaceProvisioningPhase, WorkspaceProvisioningRecoveryAction,
                WorkspaceProvisioningStatus,
            },
        },
        secrets::SecretStoreError,
        workspace_provisioning::gateway::ProvisionerWorkerError,
        workspace_resources::WorkspaceResourceError,
        workspace_setup::error::WorkspaceSetupError,
    };

    use super::*;
    use crate::workspace_provisioning::context::SyncStepResult;
    use crate::workspace_provisioning::providers::{
        WorkspaceProvisioningProvider, WorkspaceProvisioningProviderResolver,
    };
    use crate::workspace_provisioning::test_support::{
        pod, provisioning_workspace, service_parts, volume, workspace,
        FakeProvisionerWorkerGateway, FakeSecretStore, FakeWorkspaceCatalog,
        FakeWorkspaceResources,
    };

    #[derive(Debug, Clone, Default)]
    struct FakeProvisioningProvider {
        calls: Arc<Mutex<Vec<&'static str>>>,
        sync_result:
            Arc<Mutex<Option<Result<WorkspaceProvisioningResult, WorkspaceProvisioningError>>>>,
        cancel_result: Arc<Mutex<Option<Result<Workspace, WorkspaceProvisioningError>>>>,
    }

    impl FakeProvisioningProvider {
        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().expect("fake provider calls").clone()
        }

        fn push_sync_result(
            &self,
            result: Result<WorkspaceProvisioningResult, WorkspaceProvisioningError>,
        ) {
            *self.sync_result.lock().expect("fake sync result") = Some(result);
        }

        fn push_cancel_result(&self, result: Result<Workspace, WorkspaceProvisioningError>) {
            *self.cancel_result.lock().expect("fake cancel result") = Some(result);
        }
    }

    impl
        WorkspaceProvisioningProvider<
            FakeSecretStore,
            FakeWorkspaceCatalog,
            FakeProvisionerWorkerGateway,
            FakeWorkspaceResources,
        > for FakeProvisioningProvider
    {
        fn sync<'a>(
            &'a self,
            _context: &'a WorkspaceProvisioningContext<
                '_,
                FakeSecretStore,
                FakeWorkspaceCatalog,
                FakeProvisionerWorkerGateway,
                FakeWorkspaceResources,
            >,
            workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = SyncStepResult> + Send + 'a>> {
            Box::pin(async move {
                self.calls.lock().expect("fake provider calls").push("sync");
                self.sync_result
                    .lock()
                    .expect("fake sync result")
                    .take()
                    .transpose()
                    .map(|sync_result| sync_result.or_else(|| Some(result(workspace.clone()))))
            })
        }

        fn cancel<'a>(
            &'a self,
            _context: &'a WorkspaceProvisioningContext<
                '_,
                FakeSecretStore,
                FakeWorkspaceCatalog,
                FakeProvisionerWorkerGateway,
                FakeWorkspaceResources,
            >,
            workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceProvisioningError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("fake provider calls")
                    .push("cancel");
                self.cancel_result
                    .lock()
                    .expect("fake cancel result")
                    .take()
                    .unwrap_or_else(|| Ok(workspace.clone()))
            })
        }
    }

    #[derive(Debug, Clone)]
    struct FakeProvisioningProviderResolver {
        provider: FakeProvisioningProvider,
    }

    impl
        WorkspaceProvisioningProviderResolver<
            FakeSecretStore,
            FakeWorkspaceCatalog,
            FakeProvisionerWorkerGateway,
            FakeWorkspaceResources,
        > for FakeProvisioningProviderResolver
    {
        fn for_provider(
            &self,
            provider_id: &GpuCloudProviderId,
        ) -> &dyn WorkspaceProvisioningProvider<
            FakeSecretStore,
            FakeWorkspaceCatalog,
            FakeProvisionerWorkerGateway,
            FakeWorkspaceResources,
        > {
            assert_eq!(*provider_id, GpuCloudProviderId::Runpod);
            &self.provider
        }
    }

    fn service_with_provider(
        workspace: Workspace,
        provider: FakeProvisioningProvider,
    ) -> (
        WorkspaceProvisioningService<
            FakeSecretStore,
            FakeWorkspaceCatalog,
            FakeProvisionerWorkerGateway,
            FakeWorkspaceResources,
            FakeProvisioningProviderResolver,
        >,
        FakeWorkspaceCatalog,
        FakeWorkspaceResources,
    ) {
        let catalog = FakeWorkspaceCatalog::with_workspace(workspace);
        let resources = FakeWorkspaceResources::default();
        let service = WorkspaceProvisioningService::with_provider_registry(
            FakeSecretStore::with_api_key("provider-secret"),
            resources.clone(),
            catalog.clone(),
            FakeProvisionerWorkerGateway::default(),
            WorkspaceProvisioningCoordinator::default(),
            WorkspaceProvisioningConfig,
            FakeProvisioningProviderResolver { provider },
        );

        (service, catalog, resources)
    }

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
                WorkspaceProvisioningConfig,
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
            WorkspaceProvisioningConfig,
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
            WorkspaceProvisioningConfig,
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
    async fn initiate_preserves_catalog_failure_categories_without_mutation() {
        for (setup_error, expected_error) in [
            (
                WorkspaceSetupError::WorkspaceCatalogStorageUnavailable,
                WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogMigrationFailed,
                WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogQueryFailed,
                WorkspaceProvisioningError::WorkspaceCatalogQueryFailed,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogCorrupt,
                WorkspaceProvisioningError::WorkspaceCatalogCorrupt,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogSchemaMismatch,
                WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch,
            ),
        ] {
            let catalog = FakeWorkspaceCatalog::with_find_error(setup_error);
            let resources = FakeWorkspaceResources::default();
            let service = WorkspaceProvisioningService::new(
                FakeSecretStore::with_api_key("provider-secret"),
                resources.clone(),
                catalog.clone(),
                FakeProvisionerWorkerGateway::default(),
                WorkspaceProvisioningCoordinator::default(),
                WorkspaceProvisioningConfig,
            );

            let error = service
                .initiate("workspace-1")
                .await
                .expect_err("catalog failure should fail");

            assert_eq!(error, expected_error);
            assert!(catalog.updates().is_empty());
            assert!(resources.calls().is_empty());
        }
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
    async fn sync_provider_errors_persist_failure_without_retryable_metadata() {
        for (resource_error, expected_code) in [
            (
                WorkspaceResourceError::ProviderApiUnavailable,
                WorkspaceProvisioningFailureCode::ProviderApiUnavailable,
            ),
            (
                WorkspaceResourceError::ProviderRateLimited,
                WorkspaceProvisioningFailureCode::ProviderRateLimited,
            ),
            (
                WorkspaceResourceError::ProviderOperationConflict,
                WorkspaceProvisioningFailureCode::ProviderOperationConflict,
            ),
            (
                WorkspaceResourceError::ProviderRequestRejected,
                WorkspaceProvisioningFailureCode::ProviderRequestRejected,
            ),
        ] {
            let (service, _, catalog, resources, _, _) = service_parts(provisioning_workspace());
            resources.push_network_volume_result(Err(resource_error.clone()));

            let result = service
                .sync("workspace-1")
                .await
                .expect("sync should persist provider failure");

            assert_eq!(catalog.updates().len(), 1);
            assert_eq!(
                result.workspace.lifecycle_state,
                WorkspaceLifecycleState::Failed
            );
            let failure = result
                .workspace
                .last_provisioning_failure
                .expect("provider failure should be recorded");
            assert_eq!(failure.code, expected_code);
            assert_eq!(failure.source, WorkspaceProvisioningFailureSource::Provider);
            assert_eq!(failure.phase, WorkspaceProvisioningPhase::CreatingVolume);
            assert_eq!(
                catalog
                    .stored_workspace()
                    .expect("workspace should remain stored")
                    .lifecycle_state,
                WorkspaceLifecycleState::Failed
            );
        }
    }

    #[tokio::test]
    async fn sync_secure_keyring_unavailable_remains_command_level_error() {
        let provider = FakeProvisioningProvider::default();
        provider.push_sync_result(Err(WorkspaceProvisioningError::SecureKeyringUnavailable));
        let (service, catalog, _) = service_with_provider(provisioning_workspace(), provider);

        let error = service
            .sync("workspace-1")
            .await
            .expect_err("keyring outage should remain a command-level error");

        assert_eq!(error, WorkspaceProvisioningError::SecureKeyringUnavailable);
        assert!(catalog.updates().is_empty());
        let workspace = catalog
            .stored_workspace()
            .expect("workspace should remain stored");
        assert_eq!(
            workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        assert!(workspace.last_provisioning_failure.is_none());
    }

    #[tokio::test]
    async fn sync_worker_conflict_remains_command_level_error() {
        let provider = FakeProvisioningProvider::default();
        provider.push_sync_result(Err(WorkspaceProvisioningError::ProvisionerWorkerConflict));
        let (service, catalog, _) = service_with_provider(provisioning_workspace(), provider);

        let error = service
            .sync("workspace-1")
            .await
            .expect_err("worker conflict should remain a command-level error");

        assert_eq!(error, WorkspaceProvisioningError::ProvisionerWorkerConflict);
        assert!(catalog.updates().is_empty());
        let workspace = catalog
            .stored_workspace()
            .expect("workspace should remain stored");
        assert_eq!(
            workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        assert!(workspace.last_provisioning_failure.is_none());
    }

    #[tokio::test]
    async fn sync_worker_terminal_subtypes_persist_granular_failures() {
        for (worker_error, expected_code) in [
            (
                ProvisionerWorkerError::AssetDownloadFailed,
                WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed,
            ),
            (
                ProvisionerWorkerError::AssetAuthRequired,
                WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetAuthRequired,
            ),
            (
                ProvisionerWorkerError::PathValidationFailed,
                WorkspaceProvisioningFailureCode::ProvisionerWorkerPathValidationFailed,
            ),
            (
                ProvisionerWorkerError::StepTimeout,
                WorkspaceProvisioningFailureCode::ProvisionerWorkerStepTimeout,
            ),
            (
                ProvisionerWorkerError::UnexpectedError,
                WorkspaceProvisioningFailureCode::ProvisionerWorkerUnexpectedError,
            ),
        ] {
            let mut workspace = provisioning_workspace();
            workspace.persistent_storage_volume_snapshot =
                Some(volume(ProviderResourceStatus::Ready));
            workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
            let (service, _, catalog, resources, workers, _) = service_parts(workspace);
            workers.push_status_result(Err(worker_error));

            let result = service
                .sync("workspace-1")
                .await
                .expect("sync should return failed workspace state");

            assert_eq!(resources.calls(), vec!["observe_provisioning_pod"]);
            assert_eq!(
                result.workspace.lifecycle_state,
                WorkspaceLifecycleState::Failed
            );
            assert_eq!(result.progress.status, WorkspaceProvisioningStatus::Failed);

            let failure = result
                .workspace
                .last_provisioning_failure
                .expect("worker failure should be persisted");
            assert_eq!(failure.code, expected_code);
            assert_eq!(
                failure.phase,
                WorkspaceProvisioningPhase::PreparingEnvironment
            );
            assert_eq!(
                failure.source,
                WorkspaceProvisioningFailureSource::ProvisionerWorker
            );
            assert_eq!(
                failure.recovery_action,
                WorkspaceProvisioningRecoveryAction::InspectWorkspaceProvisioning
            );
            assert_eq!(
                result.progress.failure.expect("progress failure").code,
                expected_code
            );
            assert_eq!(catalog.updates().len(), 1);
        }
    }

    #[tokio::test]
    async fn sync_worker_terminal_failure_returns_catalog_error_when_failure_cannot_persist() {
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        let (service, _, catalog, _, workers, _) = service_parts(workspace);
        catalog.push_update_error(WorkspaceSetupError::WorkspaceCatalogQueryFailed);
        workers.push_status_result(Err(ProvisionerWorkerError::AssetDownloadFailed));

        let error = service
            .sync("workspace-1")
            .await
            .expect_err("catalog failure should remain a command-level error");

        assert_eq!(
            error,
            WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
        );
        assert_eq!(
            catalog
                .stored_workspace()
                .expect("workspace should still be stored")
                .lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
    }

    #[tokio::test]
    async fn sync_delegates_to_provider_capability_after_shared_checks() {
        let provider = FakeProvisioningProvider::default();
        let (service, catalog, resources) =
            service_with_provider(provisioning_workspace(), provider.clone());

        let result = service
            .sync("workspace-1")
            .await
            .expect("sync should delegate to provider");

        assert_eq!(provider.calls(), vec!["sync"]);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        assert!(catalog.updates().is_empty());
        assert!(resources.calls().is_empty());
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
        assert_eq!(catalog.updates().len(), 1);
    }

    #[tokio::test]
    async fn cancel_delegates_to_provider_capability_after_shared_checks() {
        let provider = FakeProvisioningProvider::default();
        let clean_workspace = workspace();
        provider.push_cancel_result(Ok(clean_workspace));
        let (service, catalog, resources) =
            service_with_provider(provisioning_workspace(), provider.clone());

        let result = service
            .cancel("workspace-1")
            .await
            .expect("cancel should delegate to provider");

        assert_eq!(provider.calls(), vec!["cancel"]);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Draft
        );
        assert_eq!(catalog.updates().len(), 1);
        assert!(resources.calls().is_empty());
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
