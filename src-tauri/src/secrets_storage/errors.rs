use serde::{Deserialize, Serialize};

use crate::domain::provider::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsStorageError {
    SecretRequired,
    KeyAlreadyExists,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    Provider(ProviderError),
    IdentityResponseInvalid,
}

impl From<ProviderError> for SecretsStorageError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}
