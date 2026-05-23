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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_resources::providers::runpod::test_support::*;

    #[tokio::test]
    async fn create_uses_cheapest_cpu_policy_without_selected_gpu() {
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let client = FakeRunPodClient::default();
        let base_context = context(&secrets, &catalog);
        let runpod_context = RunPodWorkspaceResourceContext::new(&base_context, &client);
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_pods(Ok(Vec::new()));
        client.push_create_pod(Ok(runpod_pod(
            "pod-1",
            ProviderResourceStatus::Running,
            Some("https://pod/status"),
        )));

        create(&runpod_context, &mut workspace)
            .await
            .expect("provisioning pod create should succeed");

        let calls = client.calls();
        let RunPodCall::CreatePod(request) = &calls[1] else {
            panic!("expected create pod call");
        };
        let payload = serde_json::to_value(request).expect("request should serialize");
        assert_eq!(payload["computeType"], "CPU");
        assert_eq!(payload["cpuFlavorIds"], serde_json::json!(["cpu3g"]));
        assert_eq!(payload["cpuFlavorPriority"], "availability");
        assert_eq!(payload["vcpuCount"], 2);
        assert!(payload.get("gpuTypeIds").is_none());
        assert_eq!(payload["dataCenterIds"], serde_json::json!(["dc-1"]));
        assert_eq!(payload["networkVolumeId"], "volume-1");
        assert_eq!(payload["volumeMountPath"], "/workspace");
        assert_eq!(payload["ports"], serde_json::json!(["8000/http"]));
    }
}
