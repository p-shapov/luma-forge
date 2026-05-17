use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{fail_workspace, is_workspace_ready, runpod_template_snapshot},
            ProviderResourceStatus, Workspace, WorkspaceLifecycleState, WorkspaceProvisioningPhase,
        },
    },
    provider_resources::{
        CreateServerlessEndpointInput, DiscoverServerlessEndpointsInput, ProviderResourceError,
        ProviderResourceGateway,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::{
    context::{SyncStepResult, WorkspaceProvisioningContext},
    failure,
    helpers::{result, serverless_endpoint_snapshot},
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
    if workspace.environment_prepared_at.is_none()
        || workspace.active_provisioning_pod_snapshot.is_some()
    {
        return Ok(None);
    }

    if workspace.serverless_endpoint_snapshot.is_none() {
        let volume = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .cloned();
        let Some(volume) = volume else {
            fail_workspace(
                workspace,
                failure::missing_provider_resource(WorkspaceProvisioningPhase::CreatingEndpoint),
            );
            let workspace = context.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        };
        let Some(template) = runpod_template_snapshot(workspace) else {
            fail_workspace(
                workspace,
                failure::readiness_validation_failed(WorkspaceProvisioningPhase::CreatingEndpoint),
            );
            let workspace = context.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        };
        let PlacementPlan::Runpod {
            selected_datacenter_id,
            selected_gpu_id,
            endpoint_keep_alive_seconds,
            ..
        } = &workspace.placement_plan;
        let selected_datacenter_id = selected_datacenter_id.clone();
        let selected_gpu_id = selected_gpu_id.clone();
        let endpoint_keep_alive_seconds = *endpoint_keep_alive_seconds;
        let discovered_endpoints = context
            .providers
            .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_endpoints.is_empty() {
            let provider_resource_ids = discovered_endpoints
                .into_iter()
                .map(|observation| observation.provider_resource_id)
                .collect();
            return context
                .fail_for_orphaned_provider_resources(
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                    provider_resource_ids,
                )
                .await;
        }
        let observation = match context
            .providers
            .create_serverless_endpoint(CreateServerlessEndpointInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                template_id: template.template_id.clone(),
                datacenter_id: selected_datacenter_id.clone(),
                selected_gpu_id: selected_gpu_id.clone(),
                network_volume_id: volume.provider_resource_id.clone(),
                endpoint_keep_alive_seconds,
            })
            .await
        {
            Ok(observation) => observation,
            Err(ProviderResourceError::ProviderOperationIndeterminate) => {
                let discovered_endpoints = context
                    .providers
                    .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_endpoints.is_empty() {
                    let provider_resource_ids = discovered_endpoints
                        .into_iter()
                        .map(|observation| observation.provider_resource_id)
                        .collect();
                    return context
                        .fail_for_orphaned_provider_resources(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingEndpoint,
                            provider_resource_ids,
                        )
                        .await;
                }
                return context
                    .fail_for_indeterminate_provider_operation(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    )
                    .await;
            }
            Err(error) => return Err(error.into()),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        context.fail_if_endpoint_status_is_terminal(workspace);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    if let Some(endpoint_id) = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        .map(|snapshot| snapshot.provider_resource_id.clone())
    {
        let observation = match context
            .providers
            .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
            .await
        {
            Ok(observation) => observation,
            Err(ProviderResourceError::ProviderResourceNotFound) => {
                return context
                    .fail_for_missing_provider_resource(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    )
                    .await;
            }
            Err(error) => return Err(error.into()),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        context.fail_if_endpoint_status_is_terminal(workspace);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    if is_workspace_ready(workspace) {
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        workspace.last_provisioning_failure = None;
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    Ok(None)
}
