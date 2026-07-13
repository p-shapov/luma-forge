#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NetworkError {
    #[error("network resource was not found")]
    NotFound,
    #[error("network request was unauthorized")]
    Unauthorized,
    #[error("network request has insufficient permissions")]
    InsufficientPermissions,
    #[error("network request was rate limited")]
    RateLimited,
    #[error("network request timed out")]
    Timeout,
    #[error("network request failed")]
    RequestFailed,
    #[error("network response was invalid")]
    InvalidResponse,
}
