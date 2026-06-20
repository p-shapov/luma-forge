use crate::domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    LifecycleOperationChanged {
        workspace_id: String,
        operation_id: String,
        trace_id: String,
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
