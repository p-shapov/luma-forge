use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    commands::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
    diagnostics::{lifecycle_log_fields, lifecycle_state_label},
    runpod_runtime::events::RunpodRuntimeEvent,
    shared::EventSink,
};

pub struct TauriRunpodRuntimeEventSink {
    app_handle: AppHandle,
}

impl TauriRunpodRuntimeEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventSink<RunpodRuntimeEvent> for TauriRunpodRuntimeEventSink {
    fn emit(&self, event: RunpodRuntimeEvent) {
        match event {
            RunpodRuntimeEvent::LifecycleOperationChanged {
                workspace_id,
                operation_id,
                diagnostic_id,
                operation,
            } => {
                let fields = lifecycle_log_fields(operation.payload.as_ref());
                let state = lifecycle_state_label(operation.state);
                let _ = LifecycleOperationChangedEvent {
                    workspace_id: workspace_id.clone(),
                    operation_id: operation_id.clone(),
                    diagnostic_id: diagnostic_id.clone(),
                    operation: operation.into(),
                }
                .emit(&self.app_handle);
                tracing::info!(
                    event = "lifecycle_operation_changed",
                    workspace_id = %workspace_id,
                    operation_id = %operation_id,
                    operation_kind = fields.operation_kind,
                    state = state,
                    step = fields.step.unwrap_or("none"),
                    diagnostic_id = diagnostic_id.as_deref().unwrap_or("none"),
                    "runtime event emitted"
                );
            }
            RunpodRuntimeEvent::WorkspaceChanged {
                workspace_id,
                workspace,
            } => {
                let _ = WorkspaceChangedEvent {
                    workspace_id: workspace_id.clone(),
                    workspace: (*workspace).into(),
                }
                .emit(&self.app_handle);
                tracing::info!(
                    event = "workspace_changed",
                    workspace_id = %workspace_id,
                    "runtime event emitted"
                );
            }
            RunpodRuntimeEvent::WorkspaceDeleted { workspace_id } => {
                let _ = WorkspaceDeletedEvent {
                    workspace_id: workspace_id.clone(),
                }
                .emit(&self.app_handle);
                tracing::info!(
                    event = "workspace_deleted",
                    workspace_id = %workspace_id,
                    "runtime event emitted"
                );
            }
        }
    }
}
