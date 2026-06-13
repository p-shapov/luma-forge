use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    commands::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
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
                let _ = LifecycleOperationChangedEvent {
                    workspace_id,
                    operation_id,
                    diagnostic_id,
                    operation: operation.into(),
                }
                .emit(&self.app_handle);
            }
            RunpodRuntimeEvent::WorkspaceChanged {
                workspace_id,
                workspace,
            } => {
                let _ = WorkspaceChangedEvent {
                    workspace_id,
                    workspace: (*workspace).into(),
                }
                .emit(&self.app_handle);
            }
            RunpodRuntimeEvent::WorkspaceDeleted { workspace_id } => {
                let _ = WorkspaceDeletedEvent { workspace_id }.emit(&self.app_handle);
            }
        }
    }
}
