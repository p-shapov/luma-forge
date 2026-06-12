pub mod hugging_face;
pub mod runpod;

use std::time::Duration;

use reqwest::StatusCode;

use crate::{secrets_storage::errors::SecretsStorageError, shared::ApiError};

pub(super) fn identity_http_client(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<reqwest::Client, SecretsStorageError> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .map_err(identity_request_error)
}

pub(super) fn identity_request_error(error: reqwest::Error) -> SecretsStorageError {
    SecretsStorageError::IdentityRequestFailed(if error.is_timeout() {
        ApiError::Timeout
    } else {
        ApiError::RequestFailed {
            message: error.to_string(),
        }
    })
}

pub(super) fn identity_response_error(error: impl ToString) -> SecretsStorageError {
    SecretsStorageError::IdentityResponseInvalid {
        message: error.to_string(),
    }
}

pub(super) fn identity_status_error(
    provider_name: &str,
    status: StatusCode,
) -> Option<SecretsStorageError> {
    if status.is_success() {
        return None;
    }

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Some(
            SecretsStorageError::IdentityRequestFailed(ApiError::Unauthorized),
        ),
        StatusCode::TOO_MANY_REQUESTS => Some(SecretsStorageError::IdentityRequestFailed(
            ApiError::RateLimited,
        )),
        _ => Some(SecretsStorageError::IdentityRequestFailed(
            ApiError::RequestFailed {
                message: format!("{provider_name} API request failed"),
            },
        )),
    }
}
