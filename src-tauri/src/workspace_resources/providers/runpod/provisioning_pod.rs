use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{fail_workspace, is_terminal_provider_resource_status},
            ProviderResourceStatus, ProvisioningPodSnapshot, Workspace, WorkspaceProvisioningPhase,
        },
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore},
    workspace_provisioning::{failure, helpers::observed_provisioning_pod_snapshot},
    workspace_resources::{
        CreateProvisioningPodInput, DiscoverProvisioningPodsInput, ObserveProvisioningPodInput,
        WorkspaceResourceError,
    },
};

use crate::workspace_resources::{WorkspaceResourceConfig, WorkspaceResourceSyncResult};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

pub(crate) async fn sync<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    if workspace.environment_prepared_at.is_none()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
    {
        let volume = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .expect("volume checked above");
        let network_volume_id = volume.provider_resource_id.clone();
        let provisioner_worker_image_ref = workspace
            .resolved_runtime_image
            .provisioner_image_ref
            .clone();
        let PlacementPlan::Runpod {
            selected_datacenter_id,
            selected_gpu_id,
            ..
        } = &workspace.placement_plan;
        let discovered_pods = context
            .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_pods.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::StartingProvisioningPod,
                discovered_pods
                    .into_iter()
                    .map(|observation| observation.provider_resource_id)
                    .collect(),
            )
            .await;
        }
        let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
            .map_err(|_| WorkspaceResourceError::ProvisionerWorkerTokenInvalid)?;
        context
            .secrets
            .write_provisioner_worker_token(&workspace.id, &token)
            .map_err(WorkspaceResourceError::from)?;
        let observation = match context
            .create_provisioning_pod(CreateProvisioningPodInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                provisioner_worker_image_ref: provisioner_worker_image_ref.clone(),
                datacenter_id: selected_datacenter_id.clone(),
                selected_gpu_id: selected_gpu_id.clone(),
                network_volume_id: network_volume_id.clone(),
                mount_path: config.volume_mount_path.clone(),
                bearer_token: token,
            })
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
                let discovered_pods = context
                    .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_pods.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::StartingProvisioningPod,
                        discovered_pods
                            .into_iter()
                            .map(|observation| observation.provider_resource_id)
                            .collect(),
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::StartingProvisioningPod,
                )
                .await;
            }
            Err(error) => {
                return handle_pod_create_error_after_token_write(context, workspace, error).await;
            }
        };
        workspace.active_provisioning_pod_snapshot = Some(ProvisioningPodSnapshot {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: observation.provider_resource_id,
            provider_resource_status: observation.provider_resource_status,
            provisioner_status_url: observation
                .provisioner_status_url
                .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?,
        });
        return context.update_workspace(workspace).await.map(Some);
    }

    if workspace.environment_prepared_at.is_some() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    let observation = match context
        .get_provisioning_pod(ObserveProvisioningPodInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: active_pod.provider_resource_id.clone(),
        })
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderResourceNotFound) => {
            return fail_for_missing_provider_resource(
                context,
                workspace,
                WorkspaceProvisioningPhase::StartingProvisioningPod,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let observed_pod = observed_provisioning_pod_snapshot(workspace, &active_pod, observation);
    if is_terminal_provider_resource_status(&observed_pod.provider_resource_status) {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::StartingProvisioningPod,
            &observed_pod.provider_resource_status,
        );
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        fail_workspace(workspace, failure);
        return context.update_workspace(workspace).await.map(Some);
    }
    if observed_pod != active_pod {
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        return context.update_workspace(workspace).await.map(Some);
    }
    if active_pod.provider_resource_status != ProviderResourceStatus::Running {
        return Ok(Some(workspace.clone()));
    }

    Ok(None)
}

pub(crate) async fn finish<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    if workspace.environment_prepared_at.is_none() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    match context
        .delete_provisioning_pod(
            workspace.gpu_cloud_provider_id,
            &active_pod.provider_resource_id,
        )
        .await
    {
        Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => {}
        Err(error) => return Err(error),
    }
    let mut terminal_pod = active_pod;
    terminal_pod.provider_resource_status = ProviderResourceStatus::Terminated;
    workspace.last_provisioning_pod_snapshot = Some(terminal_pod);
    workspace.active_provisioning_pod_snapshot = None;
    context
        .secrets
        .delete_provisioner_worker_token(&workspace.id)
        .map_err(WorkspaceResourceError::from)?;
    context.update_workspace(workspace).await.map(Some)
}

async fn handle_pod_create_error_after_token_write<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    error: WorkspaceResourceError,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let discovered_pods = match context
        .discover_provisioning_pods(DiscoverProvisioningPodsInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await
    {
        Ok(discovered_pods) => discovered_pods,
        Err(_) => return Err(error),
    };

    if !discovered_pods.is_empty() {
        return fail_for_orphaned_provider_resources(
            context,
            workspace,
            WorkspaceProvisioningPhase::StartingProvisioningPod,
            discovered_pods
                .into_iter()
                .map(|observation| observation.provider_resource_id)
                .collect(),
        )
        .await;
    }

    cleanup_worker_token_after_determinate_create_failure(context, workspace);
    Err(error)
}

fn cleanup_worker_token_after_determinate_create_failure<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &Workspace,
) where
    S: SecretStore,
{
    let _ = context
        .secrets
        .delete_provisioner_worker_token(&workspace.id);
}

async fn fail_for_indeterminate_provider_operation<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(workspace, failure::indeterminate_provider_operation(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_missing_provider_resource<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(workspace, failure::missing_provider_resource(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_orphaned_provider_resources<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
    provider_resource_ids: Vec<String>,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(
        workspace,
        failure::orphaned_provider_resources(phase, provider_resource_ids),
    );
    context.update_workspace(workspace).await.map(Some)
}

#[cfg(test)]
mod tests {
    use super::super::{
        finish_provisioning_pod_with_client, sync_provisioning_pod_with_client, test_support::*,
    };
    use crate::{
        domain::workspace::{
            ProviderResourceStatus, Workspace, WorkspaceProvisioningFailureCode,
            WorkspaceProvisioningPhase,
        },
        provider::ProviderClientError,
        secrets::SecretStoreError,
        workspace_resources::{WorkspaceResourceError, WorkspaceResourceSyncResult},
    };

    async fn sync(
        client: &FakeRunPodClient,
        workspace: &mut Workspace,
        secrets: &FakeSecretStore,
        catalog: &FakeWorkspaceCatalog,
    ) -> WorkspaceResourceSyncResult {
        let context = context(secrets, catalog);
        sync_provisioning_pod_with_client(client, &context, workspace, &config()).await
    }

    async fn finish(
        client: &FakeRunPodClient,
        workspace: &mut Workspace,
        secrets: &FakeSecretStore,
        catalog: &FakeWorkspaceCatalog,
    ) -> WorkspaceResourceSyncResult {
        let context = context(secrets, catalog);
        finish_provisioning_pod_with_client(client, &context, workspace).await
    }

    #[tokio::test]
    async fn waits_for_ready_volume() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Creating));

        let result = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed");

        assert!(result.is_none());
        assert!(client.calls().is_empty());
        assert!(secrets.write_tokens().is_empty());
    }

    #[tokio::test]
    async fn creates_token_and_pod_when_volume_is_ready() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Ok(runpod_pod(
            "pod-1",
            ProviderResourceStatus::Creating,
            Some("https://pod/status"),
        )));

        let updated = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(secrets.write_tokens().len(), 1);
        assert_eq!(
            updated
                .active_provisioning_pod_snapshot
                .expect("pod should be active")
                .provisioner_status_url,
            "https://pod/status"
        );
        match &client.calls()[1] {
            RunPodCall::CreatePod(request) => {
                assert_eq!(request.name, "luma-forge-workspace-1-provisioner");
                assert_eq!(request.volume_mount_path, "/workspace");
                assert_eq!(request.network_volume_id, "volume-1");
                assert!(request
                    .env
                    .contains_key("LUMA_FORGE_PROVISIONER_BEARER_TOKEN"));
            }
            call => panic!("unexpected call: {call:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_created_pod_without_status_url() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Ok(runpod_pod(
            "pod-1",
            ProviderResourceStatus::Creating,
            None,
        )));

        let error = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect_err("missing status url should be invalid");

        assert_eq!(error, WorkspaceResourceError::ProviderResponseInvalid);
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn determinate_pod_create_failure_deletes_worker_token_best_effort() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Err(ProviderClientError::ApiUnavailable));
        client.push_discover_pods(Ok(Vec::new()));

        let error = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect_err("pod create should fail");

        assert_eq!(error, WorkspaceResourceError::ProviderApiUnavailable);
        assert_eq!(secrets.write_tokens().len(), 1);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn token_cleanup_failure_preserves_original_pod_create_error() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        secrets.fail_delete_token(SecretStoreError::SecureKeyringUnavailable);
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Err(ProviderClientError::RateLimited));
        client.push_discover_pods(Ok(Vec::new()));

        let error = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect_err("pod create should fail");

        assert_eq!(error, WorkspaceResourceError::ProviderRateLimited);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn pod_found_after_create_error_preserves_token_and_persists_recovery_state() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Err(ProviderClientError::ResponseInvalid));
        client.push_discover_pods(Ok(vec![runpod_pod(
            "possible-pod",
            ProviderResourceStatus::Running,
            Some("https://pod/status"),
        )]));

        let updated = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("possible pod should persist recovery state")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOrphanedResources
        );
        assert!(secrets.delete_token_calls().is_empty());
    }

    #[tokio::test]
    async fn orphaned_pod_fails_before_create() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(vec![runpod_pod(
            "orphan-pod",
            ProviderResourceStatus::Running,
            Some("https://pod/status"),
        )]));

        let updated = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOrphanedResources
        );
        assert!(secrets.write_tokens().is_empty());
    }

    #[tokio::test]
    async fn indeterminate_pod_create_rediscovers_before_failing() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Err(ProviderClientError::Indeterminate));
        client.push_discover_pods(Ok(Vec::new()));

        let updated = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
        );
        assert!(secrets.delete_token_calls().is_empty());
    }

    #[tokio::test]
    async fn refreshes_active_pod_and_fails_terminal_status() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Creating));
        client.push_get_pod(Ok(runpod_pod(
            "pod-1",
            ProviderResourceStatus::Failed,
            Some("https://pod/new-status"),
        )));

        let updated = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .phase,
            WorkspaceProvisioningPhase::StartingProvisioningPod
        );
    }

    #[tokio::test]
    async fn unchanged_non_running_pod_returns_current_workspace_without_persisting() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Creating));
        client.push_get_pod(Ok(runpod_pod(
            "pod-1",
            ProviderResourceStatus::Creating,
            Some("https://pod/status"),
        )));

        let result = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed")
            .expect("current workspace should be returned");

        assert_eq!(
            result
                .active_provisioning_pod_snapshot
                .expect("pod should remain active")
                .provider_resource_status,
            ProviderResourceStatus::Creating
        );
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn prepared_environment_noops_sync() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));

        let result = sync(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("sync should succeed");

        assert!(result.is_none());
        assert!(client.calls().is_empty());
    }

    #[tokio::test]
    async fn finish_deletes_pod_moves_snapshot_and_deletes_token() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));
        client.push_delete_pod(Ok(()));

        let updated = finish(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect("finish should succeed")
            .expect("workspace should be persisted");

        assert!(updated.active_provisioning_pod_snapshot.is_none());
        assert_eq!(
            updated
                .last_provisioning_pod_snapshot
                .expect("last pod should be recorded")
                .provider_resource_status,
            ProviderResourceStatus::Terminated
        );
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn finish_tolerates_missing_pod_but_propagates_token_error() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        secrets.fail_delete_token(SecretStoreError::SecureKeyringUnavailable);
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));
        client.push_delete_pod(Err(ProviderClientError::NotFound));

        let error = finish(&client, &mut workspace, &secrets, &catalog)
            .await
            .expect_err("token delete error should propagate");

        assert_eq!(error, WorkspaceResourceError::SecureKeyringUnavailable);
        assert!(catalog.updates().is_empty());
    }
}
