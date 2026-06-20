use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    shared::EventSink,
    tauri_api::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
    workspace::events::WorkspaceEvent,
};

pub struct TauriWorkspaceEventSink {
    app_handle: AppHandle,
}

impl TauriWorkspaceEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventSink<WorkspaceEvent> for TauriWorkspaceEventSink {
    fn emit(&self, event: WorkspaceEvent) {
        match event {
            WorkspaceEvent::LifecycleOperationChanged {
                workspace_id,
                operation_id,
                trace_id,
                operation,
            } => {
                let payload = operation
                    .payload
                    .as_ref()
                    .and_then(|payload| serde_json::to_value(payload).ok());
                let operation_kind = payload
                    .as_ref()
                    .and_then(|payload| payload.get("operation"))
                    .and_then(|operation| operation.as_str())
                    .unwrap_or("unknown");
                let step = payload
                    .as_ref()
                    .and_then(|payload| payload.get("step"))
                    .and_then(|step| step.as_str())
                    .unwrap_or("none");
                let _ = LifecycleOperationChangedEvent {
                    workspace_id: workspace_id.clone(),
                    operation_id: operation_id.clone(),
                    trace_id: trace_id.clone(),
                    operation: operation.into(),
                }
                .emit(&self.app_handle);
                tracing::info!(
                    event = "lifecycle_operation_changed",
                    workspace_id = %workspace_id,
                    operation_id = %operation_id,
                    trace_id = %trace_id,
                    operation_kind = operation_kind,
                    step = step,
                    "workspace event emitted"
                );
            }
            WorkspaceEvent::WorkspaceChanged {
                workspace_id,
                workspace,
            } => {
                let _ = WorkspaceChangedEvent {
                    workspace_id: workspace_id.clone(),
                    workspace: workspace.into(),
                }
                .emit(&self.app_handle);
            }
            WorkspaceEvent::WorkspaceDeleted { workspace_id } => {
                let _ = WorkspaceDeletedEvent {
                    workspace_id: workspace_id.clone(),
                }
                .emit(&self.app_handle);
            }
        }
    }
}
