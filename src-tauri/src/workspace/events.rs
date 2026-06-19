use crate::domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceEvent {
    LifecycleOperationChanged {
        workspace_id: String,
        operation_id: String,
        diagnostic_id: Option<String>,
        operation: LifecycleOperation,
    },
    WorkspaceChanged {
        workspace_id: String,
        workspace: Box<Workspace>,
    },
    WorkspaceDeleted {
        workspace_id: String,
    },
}
