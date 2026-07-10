#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HuggingFaceError {
    #[error("Hugging Face request was unauthorized")]
    Unauthorized,
    #[error("Hugging Face request has insufficient permissions")]
    InsufficientPermissions,
    #[error("Hugging Face request was rate limited")]
    RateLimited,
    #[error("Hugging Face request timed out")]
    Timeout,
    #[error("Hugging Face request failed")]
    RequestFailed,
    #[error("Hugging Face response was invalid")]
    InvalidResponse,
}
