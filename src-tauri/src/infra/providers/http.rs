use std::time::Duration;

use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use super::ProviderError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) trait ResponseExt {
    fn provider_response(self) -> Result<Response, ProviderError>;

    async fn provider_json<T>(self) -> Result<T, ProviderError>
    where
        T: DeserializeOwned;
}

impl ResponseExt for Result<Response, reqwest::Error> {
    fn provider_response(self) -> Result<Response, ProviderError> {
        let response = self.map_err(transport_error)?;
        if let Some(error) = status_error(response.status()) {
            Err(error)
        } else {
            Ok(response)
        }
    }

    async fn provider_json<T>(self) -> Result<T, ProviderError>
    where
        T: DeserializeOwned,
    {
        self.provider_response()?
            .json()
            .await
            .map_err(|_| ProviderError::InvalidResponse)
    }
}

pub(super) fn client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| ProviderError::RequestFailed)
}

fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::RequestFailed
    }
}

fn status_error(status: StatusCode) -> Option<ProviderError> {
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
