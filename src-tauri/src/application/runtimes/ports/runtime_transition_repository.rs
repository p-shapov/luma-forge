use crate::application::{runtimes::RuntimeOperation, workspace::Workspace};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimePersistenceError {
    #[error("runtime already exists")]
    AlreadyExists,
    #[error("runtime operation is already running")]
    OperationAlreadyRunning,
    #[error("runtime was not found")]
    NotFound,
    #[error("runtime persistence is unavailable")]
    Unavailable,
    #[error("runtime persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeTransitionRepository: Send + Sync {
    async fn save_transition(
        &self,
        workspace: &Workspace,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimePersistenceError>;
}
