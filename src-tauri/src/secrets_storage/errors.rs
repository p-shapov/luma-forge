use serde::{Deserialize, Serialize};

use crate::shared::ApiError;

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
    IdentityRequestFailed(#[source] ApiError),
    #[error("api key identity response is invalid: {message}")]
    IdentityResponseInvalid { message: String },
}
