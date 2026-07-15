use serde::Serialize;
use tauri_specta::Event;

use crate::application::events::{ApplicationEvent, ApplicationEventSink};

use super::{
    RuntimeOperationDto, RuntimeOperationEvent, WorkspaceChangedEvent, WorkspaceDeletedEvent,
    WorkspaceDto,
};

pub struct TauriEventSink {
    app_handle: tauri::AppHandle,
}

impl TauriEventSink {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    fn emit<E: Event + Serialize + Clone>(&self, event: E) {
        if event.emit(&self.app_handle).is_err() {
            log::error!("tauri event emission failed");
        }
    }
}

impl ApplicationEventSink for TauriEventSink {
    fn emit(&self, event: ApplicationEvent) {
        match event {
            ApplicationEvent::WorkspaceChanged(workspace) => {
                let Ok(workspace) = WorkspaceDto::try_from(workspace) else {
                    log::error!("tauri event mapping failed");
                    return;
                };
                self.emit(WorkspaceChangedEvent { workspace });
            }
            ApplicationEvent::WorkspaceDeleted { workspace_id } => {
                self.emit(WorkspaceDeletedEvent { workspace_id });
            }
            ApplicationEvent::RuntimeOperationChanged(operation) => {
                let Ok(operation) = RuntimeOperationDto::try_from(operation) else {
                    log::error!("tauri event mapping failed");
                    return;
                };
                self.emit(RuntimeOperationEvent { operation });
            }
        }
    }
}
