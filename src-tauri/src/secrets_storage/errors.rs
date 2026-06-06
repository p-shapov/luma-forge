use serde::{Deserialize, Serialize};

use crate::domain::provider::ProviderApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsStorageError {
    SecretRequired,
    KeyAlreadyExists,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    Provider(ProviderApiError),
    IdentityResponseInvalid,
}

impl From<ProviderApiError> for SecretsStorageError {
    fn from(error: ProviderApiError) -> Self {
        Self::Provider(error)
    }
}
