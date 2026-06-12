use std::{sync::Arc, time::Duration};

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationId, LifecycleOperationState},
        runpod_runtime::{RunpodLifecycleError, RunpodProvisionStep, RunpodProvisionerStatus},
        workspace::{
            WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceRuntimeInvalidReason,
            WorkspaceState,
        },
    },
    lifecycle_journal::LifecycleJournalRepository,
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        contracts::RunpodContractResolver,
        errors::RunpodRuntimeError,
        events::RunpodRuntimeEventSink,
        provider::{
            CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
            CreateRunpodServerlessTemplateParams, RunpodRuntimeClient,
            StartRunpodProvisionerPodParams,
        },
    },
    helpers::{load_running_operation, mark_operation_state, mark_running_step, persist_workspace},
};

// Keep polling until the worker container is reachable; 12 * 5s = 60s warm-up window.
const MAX_PROVISIONER_STARTUP_PROBE_ATTEMPTS: u32 = 12;

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    workflow_catalog: &WorkflowCatalogService,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    provisioner_poll_interval: Duration,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    let operation = load_running_operation(lifecycle_journal, operation_id).await?;
    let mut workspace = match workspace_repository
        .find_workspace_by_id(&operation.workspace_id)
        .await
    {
        Ok(Some(workspace)) => workspace,
        Ok(None) | Err(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                RunpodProvisionStep::CreateNetworkVolume,
                Some(RunpodLifecycleError::InvalidRuntimeState),
            )
            .await?;
            return Ok(());
        }
    };

    let mut failed_step = RunpodProvisionStep::CreateNetworkVolume;
    let result = async {
        failed_step = RunpodProvisionStep::CreateNetworkVolume;
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        let runtime_state = runtime.clone();
        let workflow_catalog = workflow_catalog
            .get_workflow_catalog()
            .map_err(|_| RunpodRuntimeError::InvalidRuntimeState)?;
        let resolved_workflow = workflow_catalog
            .resolve(&workspace.workflow)
            .ok_or(RunpodRuntimeError::InvalidRuntimeState)?;
        let contracts = RunpodContractResolver::resolve(&resolved_workflow)?;
        let provider = runpod_client;

        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            RunpodProvisionStep::CreateNetworkVolume,
            None,
        )
        .await?;
        let volume_id = provider
            .create_network_volume(CreateRunpodNetworkVolumeParams {
                workspace_id: workspace.id.clone(),
                data_center_id: runtime_state.placement.data_center_id.clone(),
                size_gb: runtime_state.placement.volume_size_gb,
            })
            .await?;
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.network_volume_id = Some(volume_id.clone());
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        failed_step = RunpodProvisionStep::StartProvisionerPod;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            RunpodProvisionStep::StartProvisionerPod,
            None,
        )
        .await?;
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        let provisioner_id = provider
            .start_provisioner_pod(StartRunpodProvisionerPodParams {
                workspace_id: workspace.id.clone(),
                data_center_id: runtime.placement.data_center_id.clone(),
                network_volume_id: volume_id.clone(),
                provisioner_image_ref: contracts.provisioner_contract.image_ref.clone(),
                requires_hugging_face_api_key: resolved_workflow.requires_hugging_face_api_key,
                required_model_assets: resolved_workflow.required_model_assets.clone(),
            })
            .await?;
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.provisioner_pod_id = Some(provisioner_id.clone());
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        let mut provisioner_failed = false;
        let mut has_seen_initial_status = false;
        let mut startup_probe_attempts = 0u32;
        loop {
            failed_step = RunpodProvisionStep::PollProvisioner;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                RunpodProvisionStep::PollProvisioner,
                None,
            )
            .await?;
            let status = provider
                .get_provisioner_status(&workspace.id, &provisioner_id)
                .await;
            match status {
                Ok(status) => {
                    has_seen_initial_status = true;
                    startup_probe_attempts = 0;
                    match status {
                        RunpodProvisionerStatus::Pending
                        | RunpodProvisionerStatus::Starting
                        | RunpodProvisionerStatus::Running => {
                            if !provisioner_poll_interval.is_zero() {
                                tokio::time::sleep(provisioner_poll_interval).await;
                            }
                        }
                        RunpodProvisionerStatus::Succeeded => break,
                        RunpodProvisionerStatus::Failed => {
                            provisioner_failed = true;
                            break;
                        }
                    }
                }
                Err(RunpodRuntimeError::ProvisionerUnavailable)
                    if !has_seen_initial_status
                        && !provisioner_poll_interval.is_zero()
                        && startup_probe_attempts < MAX_PROVISIONER_STARTUP_PROBE_ATTEMPTS =>
                {
                    startup_probe_attempts += 1;
                    tokio::time::sleep(provisioner_poll_interval).await;
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        failed_step = RunpodProvisionStep::TerminateProvisionerPod;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            RunpodProvisionStep::TerminateProvisionerPod,
            None,
        )
        .await?;
        provider.terminate_provisioner_pod(&provisioner_id).await?;
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.provisioner_pod_id = None;
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        if provisioner_failed {
            return Err(RunpodRuntimeError::ProvisionerFailed);
        }

        failed_step = RunpodProvisionStep::CreateTemplate;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            RunpodProvisionStep::CreateTemplate,
            None,
        )
        .await?;
        let template_id = provider
            .create_serverless_template(CreateRunpodServerlessTemplateParams {
                workspace_id: workspace.id.clone(),
                endpoint_image_ref: contracts.endpoint_contract.image_ref.clone(),
            })
            .await?;
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.template_id = Some(template_id.clone());
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;
        failed_step = RunpodProvisionStep::CreateEndpoint;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            RunpodProvisionStep::CreateEndpoint,
            None,
        )
        .await?;
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        let endpoint_id = match provider
            .create_serverless_endpoint(CreateRunpodServerlessEndpointParams {
                workspace_id: workspace.id.clone(),
                data_center_id: runtime.placement.data_center_id.clone(),
                gpu_type_id: runtime.placement.gpu_type_id.clone(),
                network_volume_id: volume_id,
                template_id: template_id.clone(),
                keep_alive_limits: runtime.placement.keep_alive_limits.clone(),
            })
            .await
        {
            Ok(endpoint_id) => endpoint_id,
            Err(error) => {
                if provider.delete_template(&template_id).await.is_ok() {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.template_id = None;
                    let _ = persist_workspace(workspace_repository, event_sink, &workspace).await;
                }
                return Err(error);
            }
        };
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.endpoint_id = Some(endpoint_id);
        workspace.state = WorkspaceState::Ready;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;

        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                RunpodProvisionStep::CreateEndpoint,
                None,
            )
            .await?;
        }
        Err(error) => {
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
            workspace.state = if runtime.resources.is_empty() {
                WorkspaceState::Invalid {
                    reason: WorkspaceRuntimeInvalidReason::ProvisionFailed,
                }
            } else {
                WorkspaceState::CleanupRequired {
                    reason: WorkspaceCleanupRequiredReason::ProvisionFailed,
                }
            };
            persist_workspace(workspace_repository, event_sink, &workspace).await?;
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                failed_step,
                Some(lifecycle_error),
            )
            .await?;
        }
    }

    Ok(())
}

fn lifecycle_error_for(error: &RunpodRuntimeError) -> RunpodLifecycleError {
    match error {
        RunpodRuntimeError::RunpodSecretUnavailable => {
            RunpodLifecycleError::RunpodSecretUnavailable
        }
        RunpodRuntimeError::RunpodApiFailed(reason) => RunpodLifecycleError::RunpodApiFailed {
            reason: reason.clone(),
        },
        RunpodRuntimeError::ProvisionerUnavailable => RunpodLifecycleError::ProvisionerUnavailable,
        RunpodRuntimeError::ProvisionerResponseInvalid => {
            RunpodLifecycleError::ProvisionerResponseInvalid
        }
        RunpodRuntimeError::ProvisionerFailed => RunpodLifecycleError::ProvisionerFailed,
        RunpodRuntimeError::NetworkVolumeNotFound => RunpodLifecycleError::NetworkVolumeNotFound,
        RunpodRuntimeError::ProvisionerPodNotFound => RunpodLifecycleError::ProvisionerPodNotFound,
        RunpodRuntimeError::EndpointNotFound => RunpodLifecycleError::EndpointNotFound,
        RunpodRuntimeError::TemplateNotFound => RunpodLifecycleError::TemplateNotFound,
        RunpodRuntimeError::InvalidRuntimeState
        | RunpodRuntimeError::WorkspaceNotFound
        | RunpodRuntimeError::WorkspaceAlreadyExists
        | RunpodRuntimeError::LifecycleOperationAlreadyRunning { .. }
        | RunpodRuntimeError::StorageUnavailable => RunpodLifecycleError::InvalidRuntimeState,
    }
}
