use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase},
    },
    secrets::AsyncSecretStore,
    workspace_provisioning::{
        failure, failure::fail_workspace, helpers::persistent_storage_volume_snapshot,
    },
    workspace_resources::{
        state::is_terminal_provider_resource_status, CreateNetworkVolumeInput,
        DiscoverNetworkVolumesInput, WorkspaceResourceError,
    },
};

use crate::workspace_resources::{WorkspaceResourceConfig, WorkspaceResourceSyncResult};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

pub(crate) async fn sync<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    _config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: AsyncSecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    if workspace.persistent_storage_volume_snapshot.is_none() {
        let PlacementPlan::Runpod {
            selected_datacenter_id,
            persistent_storage_volume_size_bytes,
            ..
        } = &workspace.placement_plan;
        let selected_datacenter_id = selected_datacenter_id.clone();
        let persistent_storage_volume_size_bytes = *persistent_storage_volume_size_bytes;
        let discovered_volumes = context
            .discover_network_volumes(DiscoverNetworkVolumesInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_volumes.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingVolume,
            )
            .await;
        }
        let observation = match context
            .create_network_volume(CreateNetworkVolumeInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                datacenter_id: selected_datacenter_id,
                size_bytes: persistent_storage_volume_size_bytes,
            })
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
                let discovered_volumes = context
                    .discover_network_volumes(DiscoverNetworkVolumesInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_volumes.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::CreatingVolume,
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingVolume,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.persistent_storage_volume_snapshot =
            Some(persistent_storage_volume_snapshot(workspace, observation));
        fail_if_volume_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    let Some(volume_id) = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        .map(|snapshot| snapshot.provider_resource_id.clone())
    else {
        return Ok(None);
    };

    let observation = match context
        .get_network_volume(workspace.gpu_cloud_provider_id, &volume_id)
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderResourceNotFound) => {
            return fail_for_missing_provider_resource(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingVolume,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    workspace.persistent_storage_volume_snapshot =
        Some(persistent_storage_volume_snapshot(workspace, observation));
    fail_if_volume_status_is_terminal(workspace);
    context.update_workspace(workspace).await.map(Some)
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
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(workspace, failure::orphaned_provider_resources(phase));
    context.update_workspace(workspace).await.map(Some)
}

fn fail_if_volume_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_status.clone())
        .filter(is_terminal_provider_resource_status)
    {
        let failure =
            failure::provider_resource_failure(WorkspaceProvisioningPhase::CreatingVolume, &status);
        fail_workspace(workspace, failure);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{sync_network_volume_with_client, test_support::*};
    use crate::domain::workspace::{
        ProviderResourceStatus, WorkspaceProvisioningFailureCode, WorkspaceProvisioningPhase,
    };
    use crate::provider::ProviderClientError;
    use crate::workspace_resources::WorkspaceResourceSyncResult;

    async fn sync(
        client: &FakeRunPodClient,
        workspace: &mut crate::domain::workspace::Workspace,
        catalog: &FakeWorkspaceCatalog,
    ) -> WorkspaceResourceSyncResult {
        let secrets = FakeSecretStore::default();
        let context = context(&secrets, catalog);
        sync_network_volume_with_client(client, &context, workspace, &config()).await
    }

    #[tokio::test]
    async fn creates_and_persists_volume_when_missing() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        client.push_discover_network_volumes(Ok(Vec::new()));
        client.push_create_network_volume(Ok(runpod_volume(
            "volume-1",
            ProviderResourceStatus::Creating,
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        let volume = updated
            .persistent_storage_volume_snapshot
            .expect("volume snapshot should be recorded");
        assert_eq!(volume.provider_resource_id, "volume-1");
        assert_eq!(volume.mount_path, "/workspace");
        assert_eq!(catalog.updates().len(), 1);
        let calls = client.calls();
        assert_eq!(calls.len(), 2);
        assert!(matches!(calls[0], RunPodCall::DiscoverNetworkVolumes(_)));
        match &calls[1] {
            RunPodCall::CreateNetworkVolume(request) => {
                assert_eq!(request.name, "luma-forge-workspace-1-volume");
                assert_eq!(request.data_center_id, "dc-1");
                assert_eq!(request.size, 1);
            }
            call => panic!("unexpected call: {call:?}"),
        }
    }

    #[tokio::test]
    async fn ready_volume_is_noop() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));

        let result = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed");

        assert!(result.is_none());
        assert!(client.calls().is_empty());
        assert!(catalog.updates().is_empty());
    }

    #[tokio::test]
    async fn refreshes_non_ready_volume() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Creating));
        client
            .push_get_network_volume(Ok(runpod_volume("volume-1", ProviderResourceStatus::Ready)));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .persistent_storage_volume_snapshot
                .expect("volume should exist")
                .provider_resource_status,
            ProviderResourceStatus::Ready
        );
        assert!(matches!(client.calls()[0], RunPodCall::GetNetworkVolume(_)));
    }

    #[tokio::test]
    async fn terminal_volume_status_fails_workspace() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Creating));
        client.push_get_network_volume(Ok(runpod_volume(
            "volume-1",
            ProviderResourceStatus::Failed,
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        let failure = updated
            .last_provisioning_failure
            .expect("workspace should fail");
        assert_eq!(
            failure.code,
            WorkspaceProvisioningFailureCode::ProviderResourceFailed
        );
        assert_eq!(failure.phase, WorkspaceProvisioningPhase::CreatingVolume);
    }

    #[tokio::test]
    async fn missing_tracked_volume_fails_workspace() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Creating));
        client.push_get_network_volume(Err(ProviderClientError::NotFound));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderResourceMissing
        );
    }

    #[tokio::test]
    async fn discovered_orphaned_volume_fails_before_create() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        client.push_discover_network_volumes(Ok(vec![runpod_volume(
            "orphan-volume",
            ProviderResourceStatus::Ready,
        )]));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(client.calls().len(), 1);
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOrphanedResources
        );
    }

    #[tokio::test]
    async fn indeterminate_create_rediscovers_before_failing() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        client.push_discover_network_volumes(Ok(Vec::new()));
        client.push_create_network_volume(Err(ProviderClientError::Indeterminate));
        client.push_discover_network_volumes(Ok(Vec::new()));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(client.calls().len(), 3);
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
        );
    }
}
