use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    thiserror::Error,
    luma_diagnostic::DiagnosticCode,
)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApiError {
    #[error("api request was unauthorized")]
    Unauthorized,
    #[error("api request has insufficient permissions")]
    InsufficientPermissions,
    #[error("api request was rate limited")]
    RateLimited,
    #[error("api request timed out")]
    Timeout,
    #[error("api request failed: {message}")]
    RequestFailed { message: String },
}

pub fn map_api_transport_error<E>(
    error: reqwest::Error,
    wrap: impl FnOnce(ProviderApiError) -> E,
) -> E {
    if error.is_timeout() {
        wrap(ProviderApiError::Timeout)
    } else {
        wrap(ProviderApiError::RequestFailed {
            message: error.to_string(),
        })
    }
}

pub fn map_api_status_error<E>(
    provider_name: &str,
    status: StatusCode,
    wrap: impl FnOnce(ProviderApiError) -> E,
) -> Option<E> {
    if status.is_success() {
        return None;
    }

    let error = match status {
        StatusCode::UNAUTHORIZED => ProviderApiError::Unauthorized,
        StatusCode::FORBIDDEN => ProviderApiError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => ProviderApiError::RateLimited,
        _ => ProviderApiError::RequestFailed {
            message: format!("{provider_name} API request failed"),
        },
    };

    Some(wrap(error))
}
