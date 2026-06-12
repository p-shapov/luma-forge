use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState, WorkspaceId,
        },
        runpod_runtime::{
            RunpodCleanupStep, RunpodDeleteStep, RunpodLifecycleError,
            RunpodLifecycleOperationPayload, RunpodProvisionStep, RunpodResources,
        },
        workspace::{
            Workspace, WorkspaceCleanupRequiredReason, WorkspaceRuntimeInvalidReason,
            WorkspaceState,
        },
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::super::{
    errors::RunpodRuntimeError,
    events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
    service::map_workspace_catalog_error,
};

pub fn map_lifecycle_journal_error(
    error: LifecycleJournalError,
    workspace_id: &WorkspaceId,
) -> RunpodRuntimeError {
    match error {
        LifecycleJournalError::RunningOperationExists => {
            RunpodRuntimeError::LifecycleOperationAlreadyRunning {
                workspace_id: workspace_id.clone(),
            }
        }
        LifecycleJournalError::OperationNotFound
        | LifecycleJournalError::StorageUnavailable
        | LifecycleJournalError::QueryFailed
        | LifecycleJournalError::Corrupt
        | LifecycleJournalError::SchemaMismatch => RunpodRuntimeError::StorageUnavailable,
    }
}

pub async fn load_running_operation<L>(
    lifecycle_journal: &L,
    operation_id: &LifecycleOperationId,
) -> Result<LifecycleOperation, RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
{
    lifecycle_journal
        .list_running()
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &String::new()))?
        .into_iter()
        .find(|operation| operation.operation_id == *operation_id)
        .ok_or(RunpodRuntimeError::StorageUnavailable)
}

pub async fn mark_running_step<L, S>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    operation: &LifecycleOperation,
    step: S,
    error: Option<RunpodLifecycleError>,
) -> Result<(), RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
    S: RunpodStepPayload,
{
    mark_operation_state(
        lifecycle_journal,
        event_sink,
        operation,
        LifecycleOperationState::Running,
        step,
        error,
    )
    .await
    .map(|_| ())
}

pub async fn mark_operation_state<L, S>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    operation: &LifecycleOperation,
    state: LifecycleOperationState,
    step: S,
    error: Option<RunpodLifecycleError>,
) -> Result<LifecycleOperation, RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
    S: RunpodStepPayload,
{
    let payload = step.into_payload(error);
    let operation = lifecycle_journal
        .mark_state(&operation.operation_id, state, &payload)
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;
    event_sink.emit(RunpodRuntimeEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        operation: operation.clone(),
    });
    Ok(operation)
}

pub async fn persist_workspace<W>(
    workspace_repository: &W,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    workspace: &Workspace,
) -> Result<Workspace, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let workspace = workspace_repository
        .update_workspace(workspace)
        .await
        .map_err(map_workspace_catalog_error)?;
    event_sink.emit(RunpodRuntimeEvent::WorkspaceChanged {
        workspace_id: workspace.id.clone(),
        workspace: Box::new(workspace.clone()),
    });
    Ok(workspace)
}

pub trait RunpodStepPayload {
    fn into_payload(self, error: Option<RunpodLifecycleError>) -> LifecycleOperationPayload;
}

impl RunpodStepPayload for RunpodProvisionStep {
    fn into_payload(self, error: Option<RunpodLifecycleError>) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
            step: Some(self),
            error,
        })
    }
}

impl RunpodStepPayload for RunpodCleanupStep {
    fn into_payload(self, error: Option<RunpodLifecycleError>) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step: Some(self),
            error,
        })
    }
}

impl RunpodStepPayload for RunpodDeleteStep {
    fn into_payload(self, error: Option<RunpodLifecycleError>) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: Some(self),
            error,
        })
    }
}

pub fn interrupted_state_for_resources(resources: &RunpodResources) -> WorkspaceState {
    if resources.is_empty() {
        WorkspaceState::Invalid {
            reason: WorkspaceRuntimeInvalidReason::OperationInterrupted,
        }
    } else {
        WorkspaceState::CleanupRequired {
            reason: WorkspaceCleanupRequiredReason::OperationInterrupted,
        }
    }
}

pub fn payload_with_app_interrupted_error(
    payload: &LifecycleOperationPayload,
) -> LifecycleOperationPayload {
    match payload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
            step,
            ..
        }) => LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
            step: step.clone(),
            error: Some(RunpodLifecycleError::AppInterrupted),
        }),
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step,
            ..
        }) => LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step: step.clone(),
            error: Some(RunpodLifecycleError::AppInterrupted),
        }),
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step, ..
        }) => LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: step.clone(),
            error: Some(RunpodLifecycleError::AppInterrupted),
        }),
    }
}
