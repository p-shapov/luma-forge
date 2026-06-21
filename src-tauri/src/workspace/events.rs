use crate::domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    LifecycleOperationChanged {
        workspace_id: String,
        operation_id: String,
        operation: LifecycleOperation,
    },
    WorkspaceChanged {
        workspace_id: String,
        workspace: Workspace,
    },
    WorkspaceDeleted {
        workspace_id: String,
    },
}

pub trait WorkspaceEventSink: Send + Sync {
    fn emit(&self, event: WorkspaceEvent);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoopWorkspaceEventSink;

impl WorkspaceEventSink for NoopWorkspaceEventSink {
    fn emit(&self, _event: WorkspaceEvent) {}
}
