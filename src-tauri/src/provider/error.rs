use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderClientError {
    #[error("provider authorization failed")]
    Unauthorized,
    #[error("provider api key has insufficient permissions")]
    InsufficientPermissions,
    #[error("provider api unavailable")]
    ApiUnavailable,
    #[error("provider rate limited")]
    RateLimited,
    #[error("provider request rejected")]
    RequestRejected,
    #[error("provider response invalid")]
    ResponseInvalid,
    #[error("provider resource not found")]
    NotFound,
    #[error("provider operation conflict")]
    Conflict,
    #[error("provider operation result indeterminate")]
    Indeterminate,
}
