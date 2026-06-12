use serde::{Deserialize, Serialize};

use crate::shared::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsStorageError {
    SecretRequired,
    KeyAlreadyExists,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    IdentityRequestFailed(ApiError),
    IdentityResponseInvalid { message: String },
}
