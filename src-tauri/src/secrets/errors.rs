use serde::{Deserialize, Serialize};

use reqwest::StatusCode;

use crate::provider::errors::{map_api_status_error, map_api_transport_error, ProviderApiError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum SecretsStorageError {
    #[error("api key is required")]
    SecretRequired,
    #[error("api key is already configured")]
    KeyAlreadyExists,
    #[error("api key is not configured")]
    KeyNotFound,
    #[error("secure storage is unavailable")]
    StoreUnavailable,
    #[error("stored api key is invalid")]
    StoredSecretInvalid,
    #[error("api key identity request failed: {0}")]
    IdentityRequestFailed(#[from] ProviderApiError),
    #[error("api key identity response is invalid: {message}")]
    IdentityResponseInvalid { message: String },
}

pub fn identity_response_invalid_message(message: impl Into<String>) -> SecretsStorageError {
    SecretsStorageError::IdentityResponseInvalid {
        message: message.into(),
    }
}

pub fn identity_response_invalid_error(error: impl std::fmt::Display) -> SecretsStorageError {
    identity_response_invalid_message(error.to_string())
}

pub fn identity_request_error(error: reqwest::Error) -> SecretsStorageError {
    map_api_transport_error(error, Into::into)
}

pub fn identity_status_error(
    provider_name: &str,
    status: StatusCode,
) -> Option<SecretsStorageError> {
    map_api_status_error(provider_name, status, Into::into)
}
