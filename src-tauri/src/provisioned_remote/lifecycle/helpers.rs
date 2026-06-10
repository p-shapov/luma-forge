use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState, WorkspaceId,
        },
        provisioned_remote::{
            ProvisionedRemoteCleanupStep, ProvisionedRemoteDeleteStep,
            ProvisionedRemoteLifecycleError, ProvisionedRemoteLifecycleOperationPayload,
            ProvisionedRemoteProvisionStep, ProvisionedRemoteResources,
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
    errors::ProvisionedRemoteError,
    events::{ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
    service::map_workspace_catalog_error,
};

pub fn map_lifecycle_journal_error(
    error: LifecycleJournalError,
    workspace_id: &WorkspaceId,
) -> ProvisionedRemoteError {
    match error {
        LifecycleJournalError::RunningOperationExists => {
            ProvisionedRemoteError::LifecycleOperationAlreadyRunning {
                workspace_id: workspace_id.clone(),
            }
        }
        LifecycleJournalError::OperationNotFound
        | LifecycleJournalError::StorageUnavailable
        | LifecycleJournalError::QueryFailed
        | LifecycleJournalError::Corrupt
        | LifecycleJournalError::SchemaMismatch => ProvisionedRemoteError::StorageUnavailable,
    }
}

pub async fn load_running_operation<L>(
    lifecycle_journal: &L,
    operation_id: &LifecycleOperationId,
) -> Result<LifecycleOperation, ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
{
    lifecycle_journal
        .list_running()
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &String::new()))?
        .into_iter()
        .find(|operation| operation.operation_id == *operation_id)
        .ok_or(ProvisionedRemoteError::StorageUnavailable)
}

pub async fn mark_running_step<L, S>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    operation: &LifecycleOperation,
    step: S,
    error: Option<ProvisionedRemoteLifecycleError>,
) -> Result<(), ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
    S: ProvisionedRemoteStepPayload,
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
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    operation: &LifecycleOperation,
    state: LifecycleOperationState,
    step: S,
    error: Option<ProvisionedRemoteLifecycleError>,
) -> Result<LifecycleOperation, ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
    S: ProvisionedRemoteStepPayload,
{
    let payload = step.into_payload(error);
    let operation = lifecycle_journal
        .mark_state(&operation.operation_id, state, &payload)
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;
    event_sink.emit(ProvisionedRemoteEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        operation: operation.clone(),
    });
    Ok(operation)
}

pub async fn persist_workspace<W>(
    workspace_repository: &W,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteError>
where
    W: WorkspaceCatalogRepository,
{
    let workspace = workspace_repository
        .update_workspace(workspace)
        .await
        .map_err(map_workspace_catalog_error)?;
    event_sink.emit(ProvisionedRemoteEvent::WorkspaceChanged {
        workspace_id: workspace.id.clone(),
        workspace: Box::new(workspace.clone()),
    });
    Ok(workspace)
}

pub trait ProvisionedRemoteStepPayload {
    fn into_payload(
        self,
        error: Option<ProvisionedRemoteLifecycleError>,
    ) -> LifecycleOperationPayload;
}

impl ProvisionedRemoteStepPayload for ProvisionedRemoteProvisionStep {
    fn into_payload(
        self,
        error: Option<ProvisionedRemoteLifecycleError>,
    ) -> LifecycleOperationPayload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision {
                step: Some(self),
                error,
            },
        )
    }
}

impl ProvisionedRemoteStepPayload for ProvisionedRemoteCleanupStep {
    fn into_payload(
        self,
        error: Option<ProvisionedRemoteLifecycleError>,
    ) -> LifecycleOperationPayload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                step: Some(self),
                error,
            },
        )
    }
}

impl ProvisionedRemoteStepPayload for ProvisionedRemoteDeleteStep {
    fn into_payload(
        self,
        error: Option<ProvisionedRemoteLifecycleError>,
    ) -> LifecycleOperationPayload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: Some(self),
                error,
            },
        )
    }
}

pub fn interrupted_state_for_resources(resources: &ProvisionedRemoteResources) -> WorkspaceState {
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
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete { step, .. },
        ) => LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: step.clone(),
                error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
            },
        ),
    }
}
