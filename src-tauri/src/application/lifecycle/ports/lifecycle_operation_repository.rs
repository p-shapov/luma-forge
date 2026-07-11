use crate::application::lifecycle::LifecycleOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleOperationRepositoryError {
    #[error("lifecycle journal is unavailable")]
    Unavailable,
    #[error("lifecycle journal contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait LifecycleOperationRepository: Send + Sync {
    async fn recent(
        &self,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn recent_for_workspace(
        &self,
        workspace_id: &str,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn running(&self) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn has_running(
        &self,
        workspace_id: &str,
    ) -> Result<bool, LifecycleOperationRepositoryError>;
}
