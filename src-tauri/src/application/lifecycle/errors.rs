#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("lifecycle operation transition is invalid")]
    InvalidTransition,
    #[error("workspace already has a running lifecycle operation")]
    OperationAlreadyRunning,
}
