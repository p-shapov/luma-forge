use crate::{
    domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace},
    shared::EventSink,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunpodRuntimeEvent {
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

pub trait RunpodRuntimeEventSink: EventSink<RunpodRuntimeEvent> {}

impl<T> RunpodRuntimeEventSink for T where T: EventSink<RunpodRuntimeEvent> {}
