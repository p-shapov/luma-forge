use thiserror::Error;

use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("invalid provider api key")]
    InvalidProviderApiKey,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("workflow catalog unavailable")]
    WorkflowCatalogUnavailable,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("local storage unavailable")]
    LocalStorageUnavailable,
    #[error("invalid placement plan")]
    InvalidPlacementPlan,
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("invalid request")]
    InvalidRequest,
}

impl From<SecretStoreError> for WorkspaceSetupError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::InvalidProviderApiKey,
        }
    }
}
