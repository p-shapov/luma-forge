use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{
                endpoint_template_matches_workspace, fail_workspace,
                is_terminal_provider_resource_status, runpod_template_snapshot,
            },
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
            ServerlessEndpointSnapshot, Workspace, WorkspaceProvisioningPhase,
        },
    },
    secrets::SecretStore,
    workspace_provisioning::{failure, helpers::serverless_endpoint_snapshot},
    workspace_resources::{
        CreateEndpointTemplateInput, CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput,
        DiscoverServerlessEndpointsInput, EndpointTemplateObservation, WorkspaceResourceError,
    },
};

use crate::workspace_resources::{
    WorkspaceResourceConfig, WorkspaceResourceService, WorkspaceResourceSyncResult,
};

pub(crate) async fn sync<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    if workspace.environment_prepared_at.is_none()
        || workspace.active_provisioning_pod_snapshot.is_some()
    {
        return Ok(None);
    }

    if let Some(result) = sync_template(context, workspace, config).await? {
        return Ok(Some(result));
    }

    sync_serverless_endpoint(context, workspace).await
}

async fn sync_template<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
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
            .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
            .await
        {
            Ok(observation) => {
                workspace.provider_provisioning_snapshot =
                    Some(runpod_template_provisioning_snapshot(observation));
                let refreshed_template = runpod_template_snapshot(workspace)
                    .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?;
                if endpoint_template_matches_workspace(&refreshed_template, workspace) {
                    return context.update_workspace(workspace).await.map(Some);
                }
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                match context
                    .delete_endpoint_template(
                        workspace.gpu_cloud_provider_id,
                        &refreshed_template.template_id,
                    )
                    .await
                {
                    Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => {}
                    Err(error) => return Err(error),
                }
                workspace.provider_provisioning_snapshot = None;
                return context.update_workspace(workspace).await.map(Some);
            }
            Err(WorkspaceResourceError::ProviderResourceNotFound) => {
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                workspace.provider_provisioning_snapshot = None;
                return context.update_workspace(workspace).await.map(Some);
            }
            Err(error) => return Err(error),
        }
    }

    if template_snapshot.is_none() {
        let endpoint_worker_image_ref = workspace.resolved_runtime_image.endpoint_image_ref.clone();
        let discovered_templates = context
            .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_templates.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                discovered_templates
                    .into_iter()
                    .map(|observation| observation.template_id)
                    .collect(),
            )
            .await;
        }
        let observation = match context
            .create_endpoint_template(CreateEndpointTemplateInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                endpoint_worker_image_ref: endpoint_worker_image_ref.clone(),
                mount_path: config.volume_mount_path.clone(),
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
                if !discovered_templates.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                        discovered_templates
                            .into_iter()
                            .map(|observation| observation.template_id)
                            .collect(),
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.provider_provisioning_snapshot =
            Some(runpod_template_provisioning_snapshot(observation));
        fail_if_template_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    let Some(template) = template_snapshot
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
    else {
        return Ok(None);
    };

    let observation = match context
        .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderResourceNotFound) => {
            return fail_for_missing_provider_resource(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpointTemplate,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    workspace.provider_provisioning_snapshot =
        Some(runpod_template_provisioning_snapshot(observation));
    fail_if_template_status_is_terminal(workspace);
    context.update_workspace(workspace).await.map(Some)
}

async fn sync_serverless_endpoint<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
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
            return context.update_workspace(workspace).await.map(Some);
        };
        let Some(template) = runpod_template_snapshot(workspace) else {
            fail_workspace(
                workspace,
                failure::readiness_validation_failed(WorkspaceProvisioningPhase::CreatingEndpoint),
            );
            return context.update_workspace(workspace).await.map(Some);
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
            .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_endpoints.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpoint,
                discovered_endpoints
                    .into_iter()
                    .map(|observation| observation.provider_resource_id)
                    .collect(),
            )
            .await;
        }
        let observation = match context
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
            Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
                let discovered_endpoints = context
                    .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_endpoints.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                        discovered_endpoints
                            .into_iter()
                            .map(|observation| observation.provider_resource_id)
                            .collect(),
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        fail_if_endpoint_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    if let Some(endpoint_id) = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        .map(|snapshot| snapshot.provider_resource_id.clone())
    {
        let observation = match context
            .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderResourceNotFound) => {
                return fail_for_missing_provider_resource(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        fail_if_endpoint_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    Ok(None)
}

async fn delete_tracked_serverless_endpoint<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    let Some(endpoint) = workspace.serverless_endpoint_snapshot.clone() else {
        return Ok(None);
    };

    match context
        .delete_serverless_endpoint(
            workspace.gpu_cloud_provider_id,
            &endpoint.provider_resource_id,
        )
        .await
    {
        Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => {}
        Err(error) => return Err(error),
    }

    workspace.serverless_endpoint_snapshot = None;
    context.update_workspace(workspace).await.map(Some)
}

fn runpod_template_provisioning_snapshot(
    observation: EndpointTemplateObservation,
) -> ProviderProvisioningSnapshot {
    ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: observation.template_id,
            endpoint_worker_image_ref: observation.endpoint_worker_image_ref,
            mount_path: observation.mount_path,
            provider_resource_status: observation.provider_resource_status,
        }),
    }
}

fn _serverless_endpoint_snapshot(
    workspace: &Workspace,
    observation: crate::workspace_resources::ServerlessEndpointObservation,
) -> ServerlessEndpointSnapshot {
    serverless_endpoint_snapshot(workspace, observation)
}

async fn fail_for_indeterminate_provider_operation<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(workspace, failure::indeterminate_provider_operation(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_missing_provider_resource<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(workspace, failure::missing_provider_resource(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_orphaned_provider_resources<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
    provider_resource_ids: Vec<String>,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(
        workspace,
        failure::orphaned_provider_resources(phase, provider_resource_ids),
    );
    context.update_workspace(workspace).await.map(Some)
}

fn fail_if_template_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = runpod_template_snapshot(workspace)
        .map(|snapshot| snapshot.provider_resource_status)
        .filter(is_terminal_provider_resource_status)
    {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::CreatingEndpointTemplate,
            &status,
        );
        fail_workspace(workspace, failure);
    }
}

fn fail_if_endpoint_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_status.clone())
        .filter(is_terminal_provider_resource_status)
    {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::CreatingEndpoint,
            &status,
        );
        fail_workspace(workspace, failure);
    }
}
