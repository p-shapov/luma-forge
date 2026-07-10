use std::time::Duration;

use reqwest::StatusCode;

use super::ProviderError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) fn client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| ProviderError::RequestFailed)
}

pub(super) fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::RequestFailed
    }
}

pub(super) fn status_error(status: StatusCode) -> Option<ProviderError> {
    if status.is_success() {
        return None;
    }

    Some(match status {
        StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        StatusCode::FORBIDDEN => ProviderError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited,
        _ => ProviderError::RequestFailed,
    })
}
