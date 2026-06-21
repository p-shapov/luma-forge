use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    tauri_api::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
    workspace::events::{WorkspaceEvent, WorkspaceEventSink},
};

pub struct TauriWorkspaceEventSink {
    app_handle: AppHandle,
}

impl TauriWorkspaceEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl WorkspaceEventSink for TauriWorkspaceEventSink {
    fn emit(&self, event: WorkspaceEvent) {
        match event {
            WorkspaceEvent::LifecycleOperationChanged {
                workspace_id,
                operation_id,
                operation,
            } => {
                let _ = LifecycleOperationChangedEvent {
                    workspace_id: workspace_id.clone(),
                    operation_id: operation_id.clone(),
                    operation: operation.into(),
                }
                .emit(&self.app_handle);
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
