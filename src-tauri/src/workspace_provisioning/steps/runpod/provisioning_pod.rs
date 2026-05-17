use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            provisioning_state::{fail_workspace, is_terminal_provider_resource_status},
            ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase,
        },
    },
    provider_resources::{
        CreateProvisioningPodInput, DiscoverProvisioningPodsInput, ObserveProvisioningPodInput,
        ProviderResourceError, ProviderResourceGateway,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::{ProvisionerWorkerBearerToken, SecretStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioning::{
        context::{SyncStepResult, WorkspaceProvisioningContext},
        failure,
        helpers::{created_provisioning_pod_snapshot, observed_provisioning_pod_snapshot, result},
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
            .providers
            .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_pods.is_empty() {
            let provider_resource_ids = discovered_pods
                .into_iter()
                .map(|observation| observation.provider_resource_id)
                .collect();
            return context
                .fail_for_orphaned_provider_resources(
                    workspace,
                    WorkspaceProvisioningPhase::StartingProvisioningPod,
                    provider_resource_ids,
                )
                .await;
        }
        let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
            .map_err(|_| WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid)?;
        context
            .secrets
            .write_provisioner_worker_token(&workspace.id, &token)
            .map_err(WorkspaceProvisioningError::from)?;
        let observation = match context
            .providers
            .create_provisioning_pod(CreateProvisioningPodInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                provisioner_worker_image_ref: provisioner_worker_image_ref.clone(),
                datacenter_id: selected_datacenter_id.clone(),
                selected_gpu_id: selected_gpu_id.clone(),
                network_volume_id: network_volume_id.clone(),
                mount_path: context.config.volume_mount_path.clone(),
                bearer_token: token,
            })
            .await
        {
            Ok(observation) => observation,
            Err(ProviderResourceError::ProviderOperationIndeterminate) => {
                let discovered_pods = context
                    .providers
                    .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_pods.is_empty() {
                    let provider_resource_ids = discovered_pods
                        .into_iter()
                        .map(|observation| observation.provider_resource_id)
                        .collect();
                    return context
                        .fail_for_orphaned_provider_resources(
                            workspace,
                            WorkspaceProvisioningPhase::StartingProvisioningPod,
                            provider_resource_ids,
                        )
                        .await;
                }
                return context
                    .fail_for_indeterminate_provider_operation(
                        workspace,
                        WorkspaceProvisioningPhase::StartingProvisioningPod,
                    )
                    .await;
            }
            Err(error) => return Err(error.into()),
        };
        workspace.active_provisioning_pod_snapshot =
            Some(created_provisioning_pod_snapshot(workspace, observation)?);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    if workspace.environment_prepared_at.is_some() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    let observation = match context
        .providers
        .get_provisioning_pod(ObserveProvisioningPodInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            provider_resource_id: active_pod.provider_resource_id.clone(),
        })
        .await
    {
        Ok(observation) => observation,
        Err(ProviderResourceError::ProviderResourceNotFound) => {
            return context
                .fail_for_missing_provider_resource(
                    workspace,
                    WorkspaceProvisioningPhase::StartingProvisioningPod,
                )
                .await;
        }
        Err(error) => return Err(error.into()),
    };
    let observed_pod = observed_provisioning_pod_snapshot(workspace, &active_pod, observation);
    if is_terminal_provider_resource_status(&observed_pod.provider_resource_status) {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::StartingProvisioningPod,
            &observed_pod.provider_resource_status,
        );
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        fail_workspace(workspace, failure);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }
    if observed_pod != active_pod {
        workspace.active_provisioning_pod_snapshot = Some(observed_pod);
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }
    if active_pod.provider_resource_status != ProviderResourceStatus::Running {
        return Ok(Some(result(workspace.clone())));
    }

    Ok(None)
}

pub(crate) async fn finish<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    if workspace.environment_prepared_at.is_none() {
        return Ok(None);
    }

    let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
        return Ok(None);
    };

    match context
        .providers
        .delete_provisioning_pod(
            workspace.gpu_cloud_provider_id,
            &active_pod.provider_resource_id,
        )
        .await
    {
        Ok(()) | Err(ProviderResourceError::ProviderResourceNotFound) => {}
        Err(error) => return Err(error.into()),
    }
    let mut terminal_pod = active_pod;
    terminal_pod.provider_resource_status = ProviderResourceStatus::Terminated;
    workspace.last_provisioning_pod_snapshot = Some(terminal_pod);
    workspace.active_provisioning_pod_snapshot = None;
    context
        .secrets
        .delete_provisioner_worker_token(&workspace.id)
        .map_err(WorkspaceProvisioningError::from)?;
    let workspace = context.update_workspace(workspace).await?;
    Ok(Some(result(workspace)))
}
