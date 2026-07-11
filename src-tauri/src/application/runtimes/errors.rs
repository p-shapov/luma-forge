#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeOperationError {
    #[error("runtime operation transition is invalid")]
    InvalidTransition,
}
