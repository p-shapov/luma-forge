use crate::application::{
    lifecycle::LifecycleOperation,
    runtimes::{Runtime, RuntimeKind},
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
    LifecycleOperationChanged(LifecycleOperation),
}

pub trait ApplicationEventSink: Send + Sync {
    fn emit(&self, event: ApplicationEvent);
}
