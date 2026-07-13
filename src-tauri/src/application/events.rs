use crate::application::{runtimes::RuntimeOperation, workspace::Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationEvent {
    WorkspaceChanged(Workspace),
    WorkspaceDeleted { workspace_id: String },
    RuntimeOperationChanged(RuntimeOperation),
}

pub trait ApplicationEventSink: Send + Sync {
    fn emit(&self, event: ApplicationEvent);
}
