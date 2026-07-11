use crate::application::runtimes::ports::RuntimeTransitionRepository;

use super::super::RunpodRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeRepositoryError {
    #[error("runtime repository is unavailable")]
    Unavailable,
    #[error("runtime repository contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeRepository:
    RuntimeTransitionRepository<RunpodRuntime> + Send + Sync
{
    async fn get(
        &self,
        workspace_id: &str,
    ) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError>;
}
