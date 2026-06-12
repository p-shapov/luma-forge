use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationId, LifecycleOperationState},
        runpod::{
            RunpodCleanupStep, RunpodLifecycleError, RunpodProvisionerError,
            RunpodRuntimeStateError,
        },
        workspace::{
            WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceRuntimeInvalidReason,
            WorkspaceState,
        },
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEventSink, provider::RunpodRuntimeClient,
    },
    helpers::{
        load_running_operation, mark_operation_state, mark_running_step, persist_workspace,
        runpod_resources_are_empty,
    },
};

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
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
                RunpodCleanupStep::DeleteEndpoint,
                Some(invalid_runtime_state()),
            )
            .await?;
            return Ok(());
        }
    };
    let mut failed_step = RunpodCleanupStep::DeleteEndpoint;

    let result = async {
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        let provider = if runpod_resources_are_empty(&runtime.resources) {
            None
        } else {
            Some(runpod_client)
        };
        if let Some(endpoint_id) = runtime.resources.endpoint_id.clone() {
            failed_step = RunpodCleanupStep::DeleteEndpoint;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when endpoint exists")
                .delete_serverless_endpoint(&endpoint_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::EndpointNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.endpoint_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(template_id) = runtime.resources.template_id.clone() {
            failed_step = RunpodCleanupStep::DeleteTemplate;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when template exists")
                .delete_template(&template_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::TemplateNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.template_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(provisioner_id) = runtime.resources.provisioner_pod_id.clone() {
            failed_step = RunpodCleanupStep::TerminateProvisionerPod;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when provisioner exists")
                .terminate_provisioner_pod(&provisioner_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::ProvisionerPodNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.provisioner_pod_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(volume_id) = runtime.resources.network_volume_id.clone() {
            failed_step = RunpodCleanupStep::DeleteNetworkVolume;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when volume exists")
                .delete_network_volume(&volume_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::NetworkVolumeNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.network_volume_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        workspace.state = WorkspaceState::NotProvisioned;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;
        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(()) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                failed_step.clone(),
                None,
            )
            .await?;
        }
        Err(error) => {
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
            workspace.state = if runpod_resources_are_empty(&runtime.resources) {
                WorkspaceState::Invalid {
                    reason: WorkspaceRuntimeInvalidReason::CleanupFailed,
                }
            } else {
                WorkspaceState::CleanupRequired {
                    reason: WorkspaceCleanupRequiredReason::CleanupFailed,
                }
            };
            persist_workspace(workspace_repository, event_sink, &workspace).await?;
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                failed_step.clone(),
                Some(lifecycle_error),
            )
            .await?;
        }
    }

    Ok(())
}

pub(super) fn lifecycle_error_for(error: &RunpodRuntimeError) -> RunpodLifecycleError {
    match error {
        RunpodRuntimeError::RunpodSecretUnavailable => RunpodLifecycleError::RunPodSecretError(
            crate::secrets_storage::SecretsStorageError::KeyNotFound,
        ),
        RunpodRuntimeError::RunpodApiFailed(reason) => {
            RunpodLifecycleError::RunPodApiError(reason.clone().into())
        }
        RunpodRuntimeError::ProvisionerUnavailable => {
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Unavailable)
        }
        RunpodRuntimeError::ProvisionerResponseInvalid => {
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::ResponseInvalid)
        }
        RunpodRuntimeError::ProvisionerFailed => {
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Failed)
        }
        RunpodRuntimeError::NetworkVolumeNotFound => {
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingVolume)
        }
        RunpodRuntimeError::ProvisionerPodNotFound => RunpodLifecycleError::InvalidRuntimeState(
            RunpodRuntimeStateError::MissingProvisionerPod,
        ),
        RunpodRuntimeError::EndpointNotFound => {
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingEndpoint)
        }
        RunpodRuntimeError::TemplateNotFound => {
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingTemplate)
        }
        RunpodRuntimeError::InvalidRuntimeState
        | RunpodRuntimeError::WorkspaceNotFound
        | RunpodRuntimeError::WorkspaceAlreadyExists
        | RunpodRuntimeError::LifecycleOperationAlreadyRunning { .. }
        | RunpodRuntimeError::StorageUnavailable => invalid_runtime_state(),
    }
}

pub(super) fn invalid_runtime_state() -> RunpodLifecycleError {
    RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::Invalid)
}
