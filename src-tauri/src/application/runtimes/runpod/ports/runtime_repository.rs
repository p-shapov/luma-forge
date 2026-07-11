use crate::application::lifecycle::LifecycleOperation;

use super::super::RunpodRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeRepositoryError {
    #[error("runtime already exists")]
    AlreadyExists,
    #[error("runtime operation is already running")]
    OperationAlreadyRunning,
    #[error("runtime was not found")]
    NotFound,
    #[error("runtime repository is unavailable")]
    Unavailable,
    #[error("runtime repository contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeRepository: Send + Sync {
    async fn get(
        &self,
        workspace_id: &str,
    ) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError>;
    async fn save_transition(
        &self,
        runtime: &RunpodRuntime,
        operation: &LifecycleOperation,
    ) -> Result<(), RunpodRuntimeRepositoryError>;
}
