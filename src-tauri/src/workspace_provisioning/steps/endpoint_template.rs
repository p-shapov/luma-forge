use crate::{
    domain::workspace::{
        provisioning_state::{endpoint_template_matches_workspace, runpod_template_snapshot},
        ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase,
    },
    provider_resources::{
        CreateEndpointTemplateInput, DiscoverEndpointTemplatesInput, ProviderResourceError,
        ProviderResourceGateway,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::{
    context::{SyncStepResult, WorkspaceProvisioningContext},
    helpers::{result, runpod_template_provisioning_snapshot},
    WorkspaceProvisioningError,
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

    let template_snapshot = runpod_template_snapshot(workspace);
    if let Some(template) = template_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status == ProviderResourceStatus::Ready)
    {
        if endpoint_template_matches_workspace(template, workspace) {
            return Ok(None);
        }

        match context
            .providers
            .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
            .await
        {
            Ok(observation) => {
                workspace.provider_provisioning_snapshot =
                    Some(runpod_template_provisioning_snapshot(observation));
                let refreshed_template = runpod_template_snapshot(workspace)
                    .ok_or(WorkspaceProvisioningError::ProviderResponseInvalid)?;
                if endpoint_template_matches_workspace(&refreshed_template, workspace) {
                    let workspace = context.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                match context
                    .providers
                    .delete_endpoint_template(
                        workspace.gpu_cloud_provider_id,
                        &refreshed_template.template_id,
                    )
                    .await
                {
                    Ok(()) | Err(ProviderResourceError::ProviderResourceNotFound) => {}
                    Err(error) => return Err(error.into()),
                }
                workspace.provider_provisioning_snapshot = None;
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(ProviderResourceError::ProviderResourceNotFound) => {
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                workspace.provider_provisioning_snapshot = None;
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(error) => return Err(error.into()),
        }
    }

    if template_snapshot.is_none() {
        let endpoint_worker_image_ref = workspace.resolved_runtime_image.endpoint_image_ref.clone();
        let discovered_templates = context
            .providers
            .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_templates.is_empty() {
            let provider_resource_ids = discovered_templates
                .into_iter()
                .map(|observation| observation.template_id)
                .collect();
            return context
                .fail_for_orphaned_provider_resources(
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                    provider_resource_ids,
                )
                .await;
        }
        let observation = match context
            .providers
            .create_endpoint_template(CreateEndpointTemplateInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                endpoint_worker_image_ref: endpoint_worker_image_ref.clone(),
                mount_path: context.config.volume_mount_path.clone(),
            })
            .await
        {
            Ok(observation) => observation,
            Err(ProviderResourceError::ProviderOperationIndeterminate) => {
                let discovered_templates = context
                    .providers
                    .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_templates.is_empty() {
                    let provider_resource_ids = discovered_templates
                        .into_iter()
                        .map(|observation| observation.template_id)
                        .collect();
                    return context
                        .fail_for_orphaned_provider_resources(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                            provider_resource_ids,
                        )
                        .await;
                }
                return context
                    .fail_for_indeterminate_provider_operation(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                    )
                    .await;
            }
            Err(error) => return Err(error.into()),
        };
        workspace.provider_provisioning_snapshot =
            Some(runpod_template_provisioning_snapshot(observation));
        context.fail_if_template_status_is_terminal(workspace);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    let Some(template) = template_snapshot
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
    else {
        return Ok(None);
    };

    let observation = match context
        .providers
        .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
        .await
    {
        Ok(observation) => observation,
        Err(ProviderResourceError::ProviderResourceNotFound) => {
            return context
                .fail_for_missing_provider_resource(
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                )
                .await;
        }
        Err(error) => return Err(error.into()),
    };
    workspace.provider_provisioning_snapshot =
        Some(runpod_template_provisioning_snapshot(observation));
    context.fail_if_template_status_is_terminal(workspace);
    let workspace = context.update_workspace(workspace).await?;
    Ok(Some(result(workspace)))
}

async fn delete_tracked_serverless_endpoint<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    let Some(endpoint) = workspace.serverless_endpoint_snapshot.clone() else {
        return Ok(None);
    };

    match context
        .providers
        .delete_serverless_endpoint(
            workspace.gpu_cloud_provider_id,
            &endpoint.provider_resource_id,
        )
        .await
    {
        Ok(()) | Err(ProviderResourceError::ProviderResourceNotFound) => {}
        Err(error) => return Err(error.into()),
    }

    workspace.serverless_endpoint_snapshot = None;
    let workspace = context.update_workspace(workspace).await?;
    Ok(Some(result(workspace)))
}
