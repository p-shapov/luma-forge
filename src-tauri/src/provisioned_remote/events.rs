use crate::domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionedRemoteEvent {
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

pub trait ProvisionedRemoteEventSink: Send + Sync {
    fn emit(&self, event: ProvisionedRemoteEvent);
}

#[derive(Debug, Clone, Copy)]
pub struct NoopProvisionedRemoteEventSink;

impl ProvisionedRemoteEventSink for NoopProvisionedRemoteEventSink {
    fn emit(&self, _event: ProvisionedRemoteEvent) {}
}
