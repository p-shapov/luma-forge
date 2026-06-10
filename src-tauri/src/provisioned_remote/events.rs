use crate::{
    domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace},
    shared::EventSink,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionedRemoteEvent {
    LifecycleOperationChanged {
        workspace_id: String,
        operation_id: String,
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

pub trait ProvisionedRemoteEventSink: EventSink<ProvisionedRemoteEvent> {}

impl<T> ProvisionedRemoteEventSink for T where T: EventSink<ProvisionedRemoteEvent> {}
