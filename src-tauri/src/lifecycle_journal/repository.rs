use crate::{
    domain::lifecycle_operation::{
        LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
        LifecycleOperationState, WorkspaceId,
    },
    shared::AppFuture,
};

use super::LifecycleJournalError;

pub trait LifecycleJournalRepository: Send + Sync {
    fn create_operation<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
        payload: &'a LifecycleOperationPayload,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>>;

    fn find_running_by_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>>;

    fn list_running<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<Vec<LifecycleOperation>, LifecycleJournalError>>;

    fn latest_for_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>>;

    fn update_operation<'a>(
        &'a self,
        operation: &'a LifecycleOperation,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>>;

    fn mark_state<'a>(
        &'a self,
        operation_id: &'a LifecycleOperationId,
        state: LifecycleOperationState,
        payload: &'a LifecycleOperationPayload,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>>;
}
