use thiserror::Error;

use crate::{
    provider::ProviderClientError, secrets::SecretStoreError,
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub(crate) enum WorkspaceResourceError {
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
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
    #[error("provisioner worker token invalid")]
    ProvisionerWorkerTokenInvalid,
}

impl From<SecretStoreError> for WorkspaceResourceError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::ProviderSetupIncomplete,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

impl From<WorkspaceSetupError> for WorkspaceResourceError {
    fn from(_error: WorkspaceSetupError) -> Self {
        Self::WorkspaceCatalogUnavailable
    }
}

impl From<ProviderClientError> for WorkspaceResourceError {
    fn from(error: ProviderClientError) -> Self {
        match error {
            ProviderClientError::Unauthorized => Self::ProviderApiKeyUnauthorized,
            ProviderClientError::ApiUnavailable => Self::ProviderApiUnavailable,
            ProviderClientError::RateLimited => Self::ProviderRateLimited,
            ProviderClientError::RequestRejected => Self::ProviderRequestRejected,
            ProviderClientError::ResponseInvalid => Self::ProviderResponseInvalid,
            ProviderClientError::NotFound => Self::ProviderResourceNotFound,
            ProviderClientError::Conflict => Self::ProviderOperationConflict,
            ProviderClientError::Indeterminate => Self::ProviderOperationIndeterminate,
        }
    }
}
