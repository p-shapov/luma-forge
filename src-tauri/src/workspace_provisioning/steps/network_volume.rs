use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase},
    },
    provider_resources::{
        CreateNetworkVolumeInput, DiscoverNetworkVolumesInput, ProviderResourceError,
        ProviderResourceGateway,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::{
    context::{SyncStepResult, WorkspaceProvisioningContext},
    helpers::{persistent_storage_volume_snapshot, result},
};

pub(crate) async fn sync<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
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
            .providers
            .discover_network_volumes(DiscoverNetworkVolumesInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_volumes.is_empty() {
            let provider_resource_ids = discovered_volumes
                .into_iter()
                .map(|observation| observation.provider_resource_id)
                .collect();
            return context
                .fail_for_orphaned_provider_resources(
                    workspace,
                    WorkspaceProvisioningPhase::CreatingVolume,
                    provider_resource_ids,
                )
                .await;
        }
        let observation = match context
            .providers
            .create_network_volume(CreateNetworkVolumeInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                datacenter_id: selected_datacenter_id,
                size_bytes: persistent_storage_volume_size_bytes,
            })
            .await
        {
            Ok(observation) => observation,
            Err(ProviderResourceError::ProviderOperationIndeterminate) => {
                let discovered_volumes = context
                    .providers
                    .discover_network_volumes(DiscoverNetworkVolumesInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_volumes.is_empty() {
                    let provider_resource_ids = discovered_volumes
                        .into_iter()
                        .map(|observation| observation.provider_resource_id)
                        .collect();
                    return context
                        .fail_for_orphaned_provider_resources(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingVolume,
                            provider_resource_ids,
                        )
                        .await;
                }
                return context
                    .fail_for_indeterminate_provider_operation(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingVolume,
                    )
                    .await;
            }
            Err(error) => return Err(error.into()),
        };
        workspace.persistent_storage_volume_snapshot =
            Some(persistent_storage_volume_snapshot(workspace, observation));
        context.fail_if_volume_status_is_terminal(workspace);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
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
        .providers
        .get_network_volume(workspace.gpu_cloud_provider_id, &volume_id)
        .await
    {
        Ok(observation) => observation,
        Err(ProviderResourceError::ProviderResourceNotFound) => {
            return context
                .fail_for_missing_provider_resource(
                    workspace,
                    WorkspaceProvisioningPhase::CreatingVolume,
                )
                .await;
        }
        Err(error) => return Err(error.into()),
    };
    workspace.persistent_storage_volume_snapshot =
        Some(persistent_storage_volume_snapshot(workspace, observation));
    context.fail_if_volume_status_is_terminal(workspace);
    let workspace = context.update_workspace(workspace).await?;
    Ok(Some(result(workspace)))
}
