use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, Workspace},
    },
    secrets::AsyncSecretStore,
    workspace_resources::contracts::{
        CreateEndpointTemplateInput, DiscoverEndpointTemplatesInput, EndpointTemplateObservation,
    },
    workspace_resources::{
        state::serverless_endpoint_snapshot, CreateServerlessEndpointInput,
        DiscoverServerlessEndpointsInput, WorkspaceResourceError, WorkspaceResourceOperationResult,
    },
};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

pub(crate) async fn create<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncSecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let volume = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .cloned()
        .ok_or(WorkspaceResourceError::ProviderResourceNotFound)?;
    let template = ensure_ready_template(context, workspace).await?;
    let Some(template) = template else {
        return Ok(Some(workspace.clone()));
    };
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        endpoint_keep_alive_seconds,
        ..
    } = &workspace.placement_plan;

    let discovered_endpoints = context
        .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if !discovered_endpoints.is_empty() {
        return Err(WorkspaceResourceError::ProviderOrphanedResources);
    }

    let observation = match context
        .create_serverless_endpoint(CreateServerlessEndpointInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            template_id: template.template_id.clone(),
            datacenter_id: selected_datacenter_id.clone(),
            selected_gpu_id: selected_gpu_id.clone(),
            network_volume_id: volume.provider_resource_id,
            endpoint_keep_alive_seconds: *endpoint_keep_alive_seconds,
        })
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
            let discovered_endpoints = context
                .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                })
                .await?;
            if !discovered_endpoints.is_empty() {
                return Err(WorkspaceResourceError::ProviderOrphanedResources);
            }
            return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
        }
        Err(error) => return Err(error),
    };

    workspace.serverless_endpoint_snapshot = Some(serverless_endpoint_snapshot(
        workspace,
        observation,
        template.template_id,
    ));
    context.update_workspace(workspace).await.map(Some)
}

pub(crate) async fn observe<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncSecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let endpoint_id = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_id.clone())
        .ok_or(WorkspaceResourceError::ProviderResourceNotFound)?;

    let observation = context
        .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
        .await?;
    let template_id = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .and_then(|snapshot| {
            snapshot.provider_metadata.as_ref().map(
                |crate::domain::workspace::ServerlessEndpointProviderMetadata::Runpod {
                     template_id,
                 }| template_id.clone(),
            )
        })
        .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?;
    workspace.serverless_endpoint_snapshot = Some(serverless_endpoint_snapshot(
        workspace,
        observation,
        template_id,
    ));
    context.update_workspace(workspace).await.map(Some)
}

async fn ensure_ready_template<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &Workspace,
) -> Result<Option<EndpointTemplateObservation>, WorkspaceResourceError>
where
    S: AsyncSecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let discovered_templates = context
        .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if let Some(template) = discovered_templates
        .into_iter()
        .find(|template| endpoint_template_matches_workspace(template, workspace))
    {
        return Ok(Some(template));
    }

    let observation = match context
        .create_endpoint_template(CreateEndpointTemplateInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            endpoint_worker_image_ref: workspace.resolved_runtime_image.endpoint_image_ref.clone(),
            mount_path: workspace
                .resolved_provisioner_image
                .volume_mount_path
                .clone(),
        })
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
            let discovered_templates = context
                .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                })
                .await?;
            if discovered_templates.is_empty() {
                return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
            }
            return discovered_templates
                .into_iter()
                .find(|template| endpoint_template_matches_workspace(template, workspace))
                .map(Some)
                .ok_or(WorkspaceResourceError::ProviderOrphanedResources);
        }
        Err(error) => return Err(error),
    };

    if endpoint_template_matches_workspace(&observation, workspace) {
        Ok(Some(observation))
    } else if matches!(
        observation.provider_resource_status,
        ProviderResourceStatus::Creating
    ) {
        Ok(None)
    } else {
        Err(WorkspaceResourceError::ProviderResponseInvalid)
    }
}

fn endpoint_template_matches_workspace(
    template: &EndpointTemplateObservation,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path == workspace.resolved_provisioner_image.volume_mount_path
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|volume| {
                volume.mount_path == workspace.resolved_provisioner_image.volume_mount_path
            })
}
