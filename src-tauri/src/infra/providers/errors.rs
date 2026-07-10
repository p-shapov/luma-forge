#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("provider request was unauthorized")]
    Unauthorized,
    #[error("provider request has insufficient permissions")]
    InsufficientPermissions,
    #[error("provider request was rate limited")]
    RateLimited,
    #[error("provider request timed out")]
    Timeout,
    #[error("provider request failed")]
    RequestFailed,
    #[error("provider response was invalid")]
    InvalidResponse,
}
