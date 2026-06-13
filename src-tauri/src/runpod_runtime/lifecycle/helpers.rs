use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState,
        },
        runpod::{
            RunpodCleanupStep, RunpodDeleteStep, RunpodLifecycleOperationPayload,
            RunpodProvisionStep, RunpodResources,
        },
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::super::{
    errors::{invalid_runtime_state_error, invalid_runtime_state_message, RunpodRuntimeError},
    events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
};

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
        .map_err(invalid_runtime_state_error)?
        .into_iter()
        .find(|operation| operation.operation_id == *operation_id)
        .ok_or_else(|| invalid_runtime_state_message("running lifecycle operation was not found"))
}

pub async fn mark_running_step<L, S>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    operation: &LifecycleOperation,
    step: S,
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
) -> Result<LifecycleOperation, RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
    S: RunpodStepPayload,
{
    let payload = step.into_payload();
    let operation = lifecycle_journal
        .mark_state(&operation.operation_id, state, &payload)
        .await
        .map_err(invalid_runtime_state_error)?;
    event_sink.emit(RunpodRuntimeEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        diagnostic_id: None,
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
        .map_err(RunpodRuntimeError::from)?;
    event_sink.emit(RunpodRuntimeEvent::WorkspaceChanged {
        workspace_id: workspace.id.clone(),
        workspace: Box::new(workspace.clone()),
    });
    Ok(workspace)
}

pub trait RunpodStepPayload {
    fn into_payload(self) -> LifecycleOperationPayload;
}

impl RunpodStepPayload for RunpodProvisionStep {
    fn into_payload(self) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
            step: Some(self),
        })
    }
}

impl RunpodStepPayload for RunpodCleanupStep {
    fn into_payload(self) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step: Some(self),
        })
    }
}

impl RunpodStepPayload for RunpodDeleteStep {
    fn into_payload(self) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: Some(self),
        })
    }
}

pub fn interrupted_state_for_resources(resources: &RunpodResources) -> WorkspaceState {
    failure_state_for_resources(resources)
}

pub fn failure_state_for_resources(resources: &RunpodResources) -> WorkspaceState {
    if runpod_resources_are_empty(resources) {
        WorkspaceState::Invalid
    } else {
        WorkspaceState::CleanupRequired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodWorkspaceFailure {
    Provision,
    Cleanup,
    Delete,
}

pub async fn mark_workspace_failed<W>(
    workspace: &mut Workspace,
    workspace_repository: &W,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    _failure: RunpodWorkspaceFailure,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let failed_state = {
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        failure_state_for_resources(&runtime.resources)
    };
    workspace.state = failed_state;
    *workspace = persist_workspace(workspace_repository, event_sink, workspace).await?;
    Ok(())
}

pub fn runpod_resources_are_empty(resources: &RunpodResources) -> bool {
    resources.network_volume_id.is_none()
        && resources.provisioner_pod_id.is_none()
        && resources.endpoint_id.is_none()
        && resources.template_id.is_none()
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
        }),
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step,
            ..
        }) => LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step: step.clone(),
        }),
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step, ..
        }) => LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: step.clone(),
        }),
    }
}
