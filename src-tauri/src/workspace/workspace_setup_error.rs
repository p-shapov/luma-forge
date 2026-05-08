use thiserror::Error;

use crate::provider_setup::ProviderSetupError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
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

impl From<ProviderSetupError> for WorkspaceSetupError {
    fn from(error: ProviderSetupError) -> Self {
        match error {
            ProviderSetupError::ProviderSetupIncomplete => Self::ProviderSetupIncomplete,
            ProviderSetupError::ProviderApiUnavailable
            | ProviderSetupError::InvalidProviderApiKey
            | ProviderSetupError::ProviderIdentityUnavailable => Self::ProviderApiUnavailable,
            ProviderSetupError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            ProviderSetupError::ProviderSetupAlreadyExists => Self::InvalidRequest,
        }
    }
}
