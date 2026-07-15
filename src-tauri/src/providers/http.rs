use std::time::Duration;

use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use super::NetworkError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) trait ResponseExt {
    fn into_response(self) -> Result<Response, NetworkError>;

    async fn into_json<T>(self) -> Result<T, NetworkError>
    where
        T: DeserializeOwned;
}

impl ResponseExt for Result<Response, reqwest::Error> {
    fn into_response(self) -> Result<Response, NetworkError> {
        let response = self.map_err(transport_error)?;
        if let Some(error) = status_error(response.status()) {
            Err(error)
        } else {
            Ok(response)
        }
    }

    async fn into_json<T>(self) -> Result<T, NetworkError>
    where
        T: DeserializeOwned,
    {
        self.into_response()?
            .json()
            .await
            .map_err(|_| NetworkError::InvalidResponse)
    }
}

pub(super) fn client() -> Result<reqwest::Client, NetworkError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| NetworkError::RequestFailed)
}

fn transport_error(error: reqwest::Error) -> NetworkError {
    if error.is_timeout() {
        NetworkError::Timeout
    } else {
        NetworkError::RequestFailed
    }
}

fn status_error(status: StatusCode) -> Option<NetworkError> {
    if status.is_success() {
        return None;
    }

    Some(match status {
        StatusCode::NOT_FOUND => NetworkError::NotFound,
        StatusCode::UNAUTHORIZED => NetworkError::Unauthorized,
        StatusCode::FORBIDDEN => NetworkError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => NetworkError::RateLimited,
        _ => NetworkError::RequestFailed,
    })
}
