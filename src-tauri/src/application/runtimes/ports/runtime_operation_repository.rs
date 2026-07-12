use crate::application::runtimes::RuntimeOperation;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeOperationRepositoryError {
    #[error("runtime operation journal is unavailable")]
    Unavailable,
    #[error("runtime operation journal contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeOperationRepository: Send + Sync {
    async fn recent(
        &self,
        limit: u64,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError>;
    async fn recent_for_workspace(
        &self,
        workspace_id: &str,
        limit: u64,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError>;
    async fn running(&self) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError>;
    async fn has_running(
        &self,
        workspace_id: &str,
    ) -> Result<bool, RuntimeOperationRepositoryError>;
}
