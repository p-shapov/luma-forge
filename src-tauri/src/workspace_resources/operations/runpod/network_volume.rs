use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase},
    },
    workspace_provisioning::{failure, helpers::persistent_storage_volume_snapshot},
    workspace_resources::{
        CreateNetworkVolumeInput, DiscoverNetworkVolumesInput, WorkspaceResourceError,
    },
};

use super::{
    RunPodResourceGateway, RunPodWorkspaceResourceOperations, WorkspaceResourceConfig,
    WorkspaceResourceSyncResult,
};

pub(crate) async fn sync<S, W, G>(
    context: &RunPodWorkspaceResourceOperations<S, W, G>,
    workspace: &mut Workspace,
    _config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    G: RunPodResourceGateway,
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
            .resources
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
                discovered_volumes
                    .into_iter()
                    .map(|observation| observation.provider_resource_id)
                    .collect(),
            )
            .await;
        }
        let observation = match context
            .resources
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
                    .resources
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
                        discovered_volumes
                            .into_iter()
                            .map(|observation| observation.provider_resource_id)
                            .collect(),
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
        .resources
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

async fn fail_for_indeterminate_provider_operation<S, W, G>(
    context: &RunPodWorkspaceResourceOperations<S, W, G>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    crate::domain::workspace::provisioning_state::fail_workspace(
        workspace,
        failure::indeterminate_provider_operation(phase),
    );
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_missing_provider_resource<S, W, G>(
    context: &RunPodWorkspaceResourceOperations<S, W, G>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    crate::domain::workspace::provisioning_state::fail_workspace(
        workspace,
        failure::missing_provider_resource(phase),
    );
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_orphaned_provider_resources<S, W, G>(
    context: &RunPodWorkspaceResourceOperations<S, W, G>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
    provider_resource_ids: Vec<String>,
) -> WorkspaceResourceSyncResult
where
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    crate::domain::workspace::provisioning_state::fail_workspace(
        workspace,
        failure::orphaned_provider_resources(phase, provider_resource_ids),
    );
    context.update_workspace(workspace).await.map(Some)
}

fn fail_if_volume_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_status.clone())
        .filter(crate::domain::workspace::provisioning_state::is_terminal_provider_resource_status)
    {
        let failure =
            failure::provider_resource_failure(WorkspaceProvisioningPhase::CreatingVolume, &status);
        crate::domain::workspace::provisioning_state::fail_workspace(workspace, failure);
    }
}
