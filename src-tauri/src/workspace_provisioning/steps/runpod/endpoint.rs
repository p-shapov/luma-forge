use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{
                endpoint_template_matches_workspace, fail_workspace, is_workspace_ready,
                runpod_template_snapshot,
            },
            ProviderResourceStatus, Workspace, WorkspaceLifecycleState, WorkspaceProvisioningPhase,
        },
    },
    provider_resources::{
        CreateEndpointTemplateInput, CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput,
        DiscoverServerlessEndpointsInput, ProviderResourceError, ProviderResourceGateway,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioning::{
        context::{SyncStepResult, WorkspaceProvisioningContext},
        failure,
        helpers::{result, runpod_template_provisioning_snapshot, serverless_endpoint_snapshot},
        WorkspaceProvisioningError,
    },
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

    if let Some(result) = sync_template(context, workspace).await? {
        return Ok(Some(result));
    }

    sync_serverless_endpoint(context, workspace).await
}

async fn sync_template<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
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

async fn sync_serverless_endpoint<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
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
