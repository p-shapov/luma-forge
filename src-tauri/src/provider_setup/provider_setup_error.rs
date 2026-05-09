use thiserror::Error;

use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProviderSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider setup already exists")]
    ProviderSetupAlreadyExists,
    #[error("invalid provider api key")]
    InvalidProviderApiKey,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider identity unavailable")]
    ProviderIdentityUnavailable,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
}

impl From<SecretStoreError> for ProviderSetupError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::InvalidProviderApiKey,
        }
    }
}
