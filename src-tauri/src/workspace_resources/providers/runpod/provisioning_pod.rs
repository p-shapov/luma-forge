use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, ProvisioningPodSnapshot, Workspace},
    },
    secrets::{AsyncSecretStore, ProvisionerWorkerBearerToken},
    workspace_resources::{
        state::observed_provisioning_pod_snapshot, CreateProvisioningPodInput,
        DiscoverProvisioningPodsInput, ObserveProvisioningPodInput, WorkspaceResourceError,
        WorkspaceResourceOperationResult,
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
        .ok_or(WorkspaceResourceError::ProviderResourceNotFound)?;
    let network_volume_id = volume.provider_resource_id.clone();
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        ..
    } = &workspace.placement_plan;
    let provisioner_image = &workspace.resolved_provisioner_image;
    let discovered_pods = context
        .discover_provisioning_pods(DiscoverProvisioningPodsInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if !discovered_pods.is_empty() {
        return Err(WorkspaceResourceError::ProviderOrphanedResources);
    }

    let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
        .map_err(|_| WorkspaceResourceError::ProvisionerWorkerTokenInvalid)?;
    context
        .secrets
        .write_provisioner_worker_token(&workspace.id, &token)
        .await
        .map_err(WorkspaceResourceError::from)?;
    let observation = match context
        .create_provisioning_pod(CreateProvisioningPodInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            provisioner_worker_image_ref: provisioner_image.provisioner_worker_image_ref.clone(),
            datacenter_id: selected_datacenter_id.clone(),
            selected_gpu_id: selected_gpu_id.clone(),
            network_volume_id,
            mount_path: provisioner_image.volume_mount_path.clone(),
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
                return Err(WorkspaceResourceError::ProviderOrphanedResources);
            }
            return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
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
    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Err(WorkspaceResourceError::ProviderResourceNotFound);
    };

    let observation = context
        .get_provisioning_pod(ObserveProvisioningPodInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: active_pod.provider_resource_id.clone(),
        })
        .await?;
    let observed_pod = observed_provisioning_pod_snapshot(workspace, &active_pod, observation);
    if observed_pod != active_pod {
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        return context.update_workspace(workspace).await.map(Some);
    }
    if active_pod.provider_resource_status != ProviderResourceStatus::Running {
        return Ok(Some(workspace.clone()));
    }

    Ok(None)
}

pub(crate) async fn delete<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncSecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
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
        .await
        .map_err(WorkspaceResourceError::from)?;
    context.update_workspace(workspace).await.map(Some)
}

async fn handle_pod_create_error_after_token_write<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    error: WorkspaceResourceError,
) -> WorkspaceResourceOperationResult
where
    S: AsyncSecretStore,
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
        return Err(WorkspaceResourceError::ProviderOrphanedResources);
    }

    cleanup_worker_token_after_determinate_create_failure(context, workspace).await;
    Err(error)
}

async fn cleanup_worker_token_after_determinate_create_failure<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &Workspace,
) where
    S: AsyncSecretStore,
{
    let _ = context
        .secrets
        .delete_provisioner_worker_token(&workspace.id)
        .await;
}
