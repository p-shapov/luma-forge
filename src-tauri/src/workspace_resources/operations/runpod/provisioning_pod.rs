use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{fail_workspace, is_terminal_provider_resource_status},
            ProviderResourceStatus, ProvisioningPodSnapshot, Workspace, WorkspaceProvisioningPhase,
        },
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore},
    workspace_provisioning::{failure, helpers::observed_provisioning_pod_snapshot},
    workspace_resources::{
        CreateProvisioningPodInput, DiscoverProvisioningPodsInput, ObserveProvisioningPodInput,
        WorkspaceResourceError,
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
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
    {
        let volume = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .expect("volume checked above");
        let network_volume_id = volume.provider_resource_id.clone();
        let provisioner_worker_image_ref = workspace
            .resolved_runtime_image
            .provisioner_image_ref
            .clone();
        let PlacementPlan::Runpod {
            selected_datacenter_id,
            selected_gpu_id,
            ..
        } = &workspace.placement_plan;
        let discovered_pods = context
            .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_pods.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::StartingProvisioningPod,
                discovered_pods
                    .into_iter()
                    .map(|observation| observation.provider_resource_id)
                    .collect(),
            )
            .await;
        }
        let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
            .map_err(|_| WorkspaceResourceError::ProvisionerWorkerTokenInvalid)?;
        context
            .secrets
            .write_provisioner_worker_token(&workspace.id, &token)
            .map_err(WorkspaceResourceError::from)?;
        let observation = match context
            .create_provisioning_pod(CreateProvisioningPodInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                provisioner_worker_image_ref: provisioner_worker_image_ref.clone(),
                datacenter_id: selected_datacenter_id.clone(),
                selected_gpu_id: selected_gpu_id.clone(),
                network_volume_id: network_volume_id.clone(),
                mount_path: config.volume_mount_path.clone(),
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
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::StartingProvisioningPod,
                        discovered_pods
                            .into_iter()
                            .map(|observation| observation.provider_resource_id)
                            .collect(),
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::StartingProvisioningPod,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.active_provisioning_pod_snapshot = Some(ProvisioningPodSnapshot {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: observation.provider_resource_id,
            provider_resource_status: observation.provider_resource_status,
            provisioner_status_url: observation
                .provisioner_status_url
                .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?,
        });
        return context.update_workspace(workspace).await.map(Some);
    }

    if workspace.environment_prepared_at.is_some() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    let observation = match context
        .get_provisioning_pod(ObserveProvisioningPodInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: active_pod.provider_resource_id.clone(),
        })
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderResourceNotFound) => {
            return fail_for_missing_provider_resource(
                context,
                workspace,
                WorkspaceProvisioningPhase::StartingProvisioningPod,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let observed_pod = observed_provisioning_pod_snapshot(workspace, &active_pod, observation);
    if is_terminal_provider_resource_status(&observed_pod.provider_resource_status) {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::StartingProvisioningPod,
            &observed_pod.provider_resource_status,
        );
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        fail_workspace(workspace, failure);
        return context.update_workspace(workspace).await.map(Some);
    }
    if observed_pod != active_pod {
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        return context.update_workspace(workspace).await.map(Some);
    }
    if active_pod.provider_resource_status != ProviderResourceStatus::Running {
        return Ok(Some(workspace.clone()));
    }

    Ok(None)
}

pub(crate) async fn finish<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    if workspace.environment_prepared_at.is_none() {
        return Ok(None);
    }

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
        .map_err(WorkspaceResourceError::from)?;
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_indeterminate_provider_operation<S, W>(
    context: &WorkspaceResourceService<S, W>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
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
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    fail_workspace(
        workspace,
        failure::orphaned_provider_resources(phase, provider_resource_ids),
    );
    context.update_workspace(workspace).await.map(Some)
}
