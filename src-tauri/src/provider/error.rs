use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderClientError {
    #[error("provider authorization failed")]
    Unauthorized,
    #[error("provider api unavailable")]
    ApiUnavailable,
    #[error("provider response invalid")]
    ResponseInvalid,
}
