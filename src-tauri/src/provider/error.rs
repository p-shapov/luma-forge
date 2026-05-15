use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderClientError {
    #[error("provider authorization failed")]
    Unauthorized,
    #[error("provider api unavailable")]
    ApiUnavailable,
    #[error("provider response invalid")]
    ResponseInvalid,
    #[error("provider resource not found")]
    NotFound,
    #[error("provider operation conflict")]
    Conflict,
    #[error("provider operation result indeterminate")]
    Indeterminate,
}
