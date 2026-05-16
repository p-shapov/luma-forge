use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SecretStoreError {
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("invalid stored provider api key")]
    InvalidStoredProviderApiKey,
    #[error("invalid stored provisioner worker token")]
    InvalidStoredProvisionerWorkerToken,
}
