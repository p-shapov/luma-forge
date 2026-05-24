use crate::{
    domain::{placement::PlacementPlan, workspace::Workspace},
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore},
    workspace_resources::{
        state::persistent_storage_volume_snapshot, CreateNetworkVolumeInput,
        DiscoverNetworkVolumesInput, WorkspaceResourceError, WorkspaceResourceOperationResult,
    },
};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

pub(crate) const RUNPOD_POD_NETWORK_VOLUME_MOUNT_PATH: &str = "/workspace";

pub(crate) async fn create<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        persistent_storage_volume_size_bytes,
        ..
    } = &workspace.placement_plan;

    let discovered_volumes = context
        .discover_network_volumes(DiscoverNetworkVolumesInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if !discovered_volumes.is_empty() {
        return Err(WorkspaceResourceError::ProviderOrphanedResources);
    }

    let observation = match context
        .create_network_volume(CreateNetworkVolumeInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            datacenter_id: selected_datacenter_id.clone(),
            size_bytes: *persistent_storage_volume_size_bytes,
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
                return Err(WorkspaceResourceError::ProviderOrphanedResources);
            }
            return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
        }
        Err(error) => return Err(error),
    };
    workspace.persistent_storage_volume_snapshot =
        Some(persistent_storage_volume_snapshot(workspace, observation));
    context.update_workspace(workspace).await.map(Some)
}

pub(crate) async fn observe<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let Some(volume_id) = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_id.clone())
    else {
        return Err(WorkspaceResourceError::ProviderResourceNotFound);
    };

    let observation = context
        .get_network_volume(workspace.gpu_cloud_provider_id, &volume_id)
        .await?;
    workspace.persistent_storage_volume_snapshot =
        Some(persistent_storage_volume_snapshot(workspace, observation));
    context.update_workspace(workspace).await.map(Some)
}
