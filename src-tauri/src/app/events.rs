use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    commands::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
    diagnostics::lifecycle_log_fields,
    shared::EventSink,
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
                diagnostic_id,
                operation,
            } => {
                let fields = lifecycle_log_fields(operation.payload.as_ref());
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
                    step = fields.step.unwrap_or("none"),
                    diagnostic_id = diagnostic_id.as_deref().unwrap_or("none"),
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
