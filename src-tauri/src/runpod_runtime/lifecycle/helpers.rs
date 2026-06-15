use std::sync::Arc;

use crate::{
    diagnostics::{lifecycle_error, lifecycle_log_fields, lifecycle_state_label},
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
    shared::EventSink,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::super::{
    errors::{invalid_runtime_state_error, invalid_runtime_state_message, RunpodRuntimeError},
    events::RunpodRuntimeEvent,
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
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
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
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
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
    let fields = lifecycle_log_fields(&operation.payload);
    tracing::info!(
        workspace_id = %operation.workspace_id,
        operation_id = %operation.operation_id,
        operation_kind = fields.operation_kind,
        state = lifecycle_state_label(operation.state),
        step = fields.step.unwrap_or("none"),
        "lifecycle operation changed"
    );
    event_sink.emit(RunpodRuntimeEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        diagnostic_id: None,
        operation: operation.clone(),
    });
    Ok(operation)
}

pub async fn mark_operation_failed<L, S>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
    operation: &LifecycleOperation,
    step: S,
    error: &RunpodRuntimeError,
) -> Result<LifecycleOperation, RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
    S: RunpodStepPayload,
{
    let payload = step.into_payload();
    let diagnostic_id = lifecycle_error(
        &operation.operation_id,
        Some(&operation.workspace_id),
        Some(&payload),
        error,
    );
    let operation = lifecycle_journal
        .mark_state(
            &operation.operation_id,
            LifecycleOperationState::Failed,
            &payload,
        )
        .await
        .map_err(invalid_runtime_state_error)?;
    event_sink.emit(RunpodRuntimeEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        diagnostic_id: Some(diagnostic_id),
        operation: operation.clone(),
    });
    Ok(operation)
}

pub async fn persist_workspace<W>(
    workspace_catalog: &W,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
    workspace: &Workspace,
) -> Result<Workspace, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let workspace = workspace_catalog
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

pub async fn mark_workspace_failed<W>(
    workspace: &mut Workspace,
    workspace_catalog: &W,
    event_sink: &Arc<dyn EventSink<RunpodRuntimeEvent>>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    let failed_state = {
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        failure_state_for_resources(&runtime.resources)
    };
    workspace.state = failed_state;
    *workspace = persist_workspace(workspace_catalog, event_sink, workspace).await?;
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use time::OffsetDateTime;

    use super::*;
    use crate::{
        lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
        shared::{AppFuture, EventSink},
    };

    #[tokio::test]
    async fn mark_operation_failed_emits_diagnostic_id() {
        let operation = LifecycleOperation {
            operation_id: "operation-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Running,
            payload: LifecycleOperationPayload::Runpod(
                RunpodLifecycleOperationPayload::Provision { step: None },
            ),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        };
        let lifecycle_journal = FakeLifecycleJournal::new(operation.clone());
        let event_sink = Arc::new(FakeEventSink::default());
        let error = RunpodRuntimeError::InvalidRuntimeState {
            message: "provider failed".to_string(),
        };

        mark_operation_failed(
            &lifecycle_journal,
            &(event_sink.clone() as Arc<dyn EventSink<RunpodRuntimeEvent>>),
            &operation,
            RunpodProvisionStep::CreateNetworkVolume,
            &error,
        )
        .await
        .expect("failure should be recorded");

        let diagnostic_id = event_sink
            .last_lifecycle_diagnostic_id()
            .expect("failed lifecycle event should include diagnostic id");
        assert!(diagnostic_id.starts_with("diag-"));
    }

    #[derive(Clone)]
    struct FakeLifecycleJournal {
        operation: Arc<Mutex<LifecycleOperation>>,
    }

    impl FakeLifecycleJournal {
        fn new(operation: LifecycleOperation) -> Self {
            Self {
                operation: Arc::new(Mutex::new(operation)),
            }
        }
    }

    impl LifecycleJournalRepository for FakeLifecycleJournal {
        fn create_operation<'a>(
            &'a self,
            _workspace_id: &'a String,
            _payload: &'a LifecycleOperationPayload,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async { Err(LifecycleJournalError::OperationNotFound) })
        }

        fn find_running_by_workspace<'a>(
            &'a self,
            _workspace_id: &'a String,
        ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async { Ok(None) })
        }

        fn list_running<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<Vec<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn latest_for_workspace<'a>(
            &'a self,
            _workspace_id: &'a String,
        ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
            Box::pin(async { Ok(None) })
        }

        fn delete_for_workspace<'a>(
            &'a self,
            _workspace_id: &'a String,
        ) -> AppFuture<'a, Result<(), LifecycleJournalError>> {
            Box::pin(async { Ok(()) })
        }

        fn update_operation<'a>(
            &'a self,
            _operation: &'a LifecycleOperation,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async { Err(LifecycleJournalError::OperationNotFound) })
        }

        fn mark_state<'a>(
            &'a self,
            _operation_id: &'a LifecycleOperationId,
            state: LifecycleOperationState,
            payload: &'a LifecycleOperationPayload,
        ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
            Box::pin(async move {
                let mut operation = self.operation.lock().expect("operation state");
                operation.state = state;
                operation.payload = payload.clone();
                Ok(operation.clone())
            })
        }
    }

    #[derive(Default)]
    struct FakeEventSink {
        events: Mutex<Vec<RunpodRuntimeEvent>>,
    }

    impl FakeEventSink {
        fn last_lifecycle_diagnostic_id(&self) -> Option<String> {
            self.events
                .lock()
                .expect("event state")
                .iter()
                .rev()
                .find_map(|event| match event {
                    RunpodRuntimeEvent::LifecycleOperationChanged { diagnostic_id, .. } => {
                        diagnostic_id.clone()
                    }
                    _ => None,
                })
        }
    }

    impl EventSink<RunpodRuntimeEvent> for FakeEventSink {
        fn emit(&self, event: RunpodRuntimeEvent) {
            self.events.lock().expect("event state").push(event);
        }
    }
}
