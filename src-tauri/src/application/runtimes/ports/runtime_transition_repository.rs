use crate::application::runtimes::{RuntimeModel, RuntimeOperation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeTransitionRepositoryError {
    #[error("runtime already exists")]
    AlreadyExists,
    #[error("runtime operation is already running")]
    OperationAlreadyRunning,
    #[error("runtime was not found")]
    NotFound,
    #[error("runtime transition persistence is unavailable")]
    Unavailable,
    #[error("runtime transition persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeTransitionRepository<R: RuntimeModel>: Send + Sync {
    async fn save_transition(
        &self,
        runtime: &R,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError>;
}
