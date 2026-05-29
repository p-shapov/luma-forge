use thiserror::Error;

use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProviderSetupError {
    #[error("provider setup not found")]
    ProviderSetupNotFound,
    #[error("provider setup already exists")]
    ProviderSetupAlreadyExists,
    #[error("provider api key is required")]
    ProviderApiKeyRequired,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("stored provider api key invalid")]
    StoredProviderApiKeyInvalid,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider identity response invalid")]
    ProviderIdentityResponseInvalid,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("provider setup recovery required")]
    ProviderSetupRecoveryRequired,
}

impl From<SecretStoreError> for ProviderSetupError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::StoredProviderApiKeyInvalid,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredHuggingFaceApiKey => Self::SecureKeyringUnavailable,
        }
    }
}
