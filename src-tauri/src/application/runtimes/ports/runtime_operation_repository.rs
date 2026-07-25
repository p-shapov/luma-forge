use crate::application::runtimes::RuntimeOperation;
use uuid::Uuid;

#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeOperationRepositoryError {
    #[error("runtime operation journal is unavailable")]
    Unavailable,
    #[error("runtime operation journal contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeOperationRepository: Send + Sync {
    async fn get(
        &self,
        id: Uuid,
    ) -> Result<Option<RuntimeOperation>, RuntimeOperationRepositoryError>;
    async fn page(
        &self,
        workspace_id: Option<&str>,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<RuntimeOperation>, u64), RuntimeOperationRepositoryError>;
    async fn running(&self) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError>;
    async fn has_running(
        &self,
        workspace_id: &str,
    ) -> Result<bool, RuntimeOperationRepositoryError>;
}
