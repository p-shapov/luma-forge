use crate::application::{
    runtimes::{Runtime, RuntimeKind, RuntimeOperation},
    workspace::Workspace,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationEvent {
    WorkspaceChanged(Workspace),
    WorkspaceDeleted {
        workspace_id: String,
    },
    RuntimeChanged(Runtime),
    RuntimeDeleted {
        workspace_id: String,
        kind: RuntimeKind,
    },
    RuntimeOperationChanged(RuntimeOperation),
}

pub trait ApplicationEventSink: Send + Sync {
    fn emit(&self, event: ApplicationEvent);
}
