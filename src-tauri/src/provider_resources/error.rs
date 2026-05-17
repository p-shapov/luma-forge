use thiserror::Error;

use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProviderResourceError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider rate limited")]
    ProviderRateLimited,
    #[error("provider request rejected")]
    ProviderRequestRejected,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider resource not found")]
    ProviderResourceNotFound,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
}

impl From<SecretStoreError> for ProviderResourceError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::ProviderSetupIncomplete,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => Self::SecureKeyringUnavailable,
        }
    }
}
