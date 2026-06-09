use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    commands::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    },
    provisioned_remote::events::{ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
};

pub struct TauriProvisionedRemoteEventSink {
    app_handle: AppHandle,
}

impl TauriProvisionedRemoteEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl ProvisionedRemoteEventSink for TauriProvisionedRemoteEventSink {
    fn emit(&self, event: ProvisionedRemoteEvent) {
        match event {
            ProvisionedRemoteEvent::LifecycleOperationChanged {
                workspace_id,
                operation_id,
                operation,
            } => {
                let _ = LifecycleOperationChangedEvent {
                    workspace_id,
                    operation_id,
                    operation: operation.into(),
                }
                .emit(&self.app_handle);
            }
            ProvisionedRemoteEvent::WorkspaceChanged {
                workspace_id,
                workspace,
            } => {
                let _ = WorkspaceChangedEvent {
                    workspace_id,
                    workspace: (*workspace).into(),
                }
                .emit(&self.app_handle);
            }
            ProvisionedRemoteEvent::WorkspaceDeleted { workspace_id } => {
                let _ = WorkspaceDeletedEvent { workspace_id }.emit(&self.app_handle);
            }
        }
    }
}
