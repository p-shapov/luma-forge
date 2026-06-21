use crate::domain::{
    lifecycle_operation::{
        LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
        LifecycleOperationState,
    },
    workspace::WorkspaceId,
};

use super::LifecycleJournalError;

#[async_trait::async_trait]
pub trait LifecycleJournalRepository: Send + Sync {
    async fn create_operation(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<LifecycleOperation, LifecycleJournalError>;

    async fn find_running_by_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<LifecycleOperation>, LifecycleJournalError>;

    async fn list_running(&self) -> Result<Vec<LifecycleOperation>, LifecycleJournalError>;

    async fn latest_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<LifecycleOperation>, LifecycleJournalError>;

    async fn delete_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(), LifecycleJournalError>;

    async fn update_operation(
        &self,
        operation: &LifecycleOperation,
    ) -> Result<LifecycleOperation, LifecycleJournalError>;

    async fn mark_state(
        &self,
        operation_id: &LifecycleOperationId,
        state: LifecycleOperationState,
        payload: Option<&LifecycleOperationPayload>,
    ) -> Result<LifecycleOperation, LifecycleJournalError>;
}
