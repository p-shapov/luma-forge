use tauri::AppHandle;
use tauri_specta::Event;

use crate::{
    commands::types::workspace::{
        LifecycleOperationChangedEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
        WorkspaceResponse,
    },
    provisioned_remote::events::ProvisionedRemoteEvent,
    shared::EventSink,
    workflow_catalog::WorkflowCatalogService,
};

pub struct TauriProvisionedRemoteEventSink {
    app_handle: AppHandle,
}

impl TauriProvisionedRemoteEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventSink<ProvisionedRemoteEvent> for TauriProvisionedRemoteEventSink {
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
                let workspace = *workspace;
                let event = WorkflowCatalogService::new()
                    .get_workflow_catalog()
                    .ok()
                    .and_then(|catalog| catalog.resolve(&workspace.workflow))
                    .map(|workflow| WorkspaceChangedEvent {
                        workspace_id,
                        workspace: WorkspaceResponse::from_parts(workspace, workflow),
                    });
                if let Some(event) = event {
                    let _ = event.emit(&self.app_handle);
                }
            }
            ProvisionedRemoteEvent::WorkspaceDeleted { workspace_id } => {
                let _ = WorkspaceDeletedEvent { workspace_id }.emit(&self.app_handle);
            }
        }
    }
}
