use thiserror::Error;

use crate::{
    provider::{runpod::RunPodHttpClientInitError, ProviderClientError},
    secrets::SecretStoreError,
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub(crate) enum WorkspaceResourceError {
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("workspace catalog storage unavailable")]
    WorkspaceCatalogStorageUnavailable,
    #[error("workspace catalog migration failed")]
    WorkspaceCatalogMigrationFailed,
    #[error("workspace catalog query failed")]
    WorkspaceCatalogQueryFailed,
    #[error("workspace catalog corrupt")]
    WorkspaceCatalogCorrupt,
    #[error("workspace catalog schema mismatch")]
    WorkspaceCatalogSchemaMismatch,
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
    #[error("provider orphaned resources")]
    ProviderOrphanedResources,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("hugging face api key setup is required")]
    HuggingFaceApiKeySetupRequired,
    #[error("resource cleanup failed")]
    CleanupFailed,
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
            SecretStoreError::InvalidStoredHuggingFaceApiKey => Self::SecureKeyringUnavailable,
        }
    }
}

impl From<WorkspaceSetupError> for WorkspaceResourceError {
    fn from(error: WorkspaceSetupError) -> Self {
        match error {
            WorkspaceSetupError::WorkspaceCatalogUnavailable => Self::WorkspaceCatalogUnavailable,
            WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
                Self::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
                Self::WorkspaceCatalogMigrationFailed
            }
            WorkspaceSetupError::WorkspaceCatalogQueryFailed => Self::WorkspaceCatalogQueryFailed,
            WorkspaceSetupError::WorkspaceCatalogCorrupt => Self::WorkspaceCatalogCorrupt,
            WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
                Self::WorkspaceCatalogSchemaMismatch
            }
            _ => Self::WorkspaceCatalogUnavailable,
        }
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

impl From<RunPodHttpClientInitError> for WorkspaceResourceError {
    fn from(_error: RunPodHttpClientInitError) -> Self {
        Self::ProviderApiUnavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_setup_catalog_errors_preserve_resource_categories() {
        for (setup_error, expected) in [
            (
                WorkspaceSetupError::WorkspaceCatalogUnavailable,
                WorkspaceResourceError::WorkspaceCatalogUnavailable,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogStorageUnavailable,
                WorkspaceResourceError::WorkspaceCatalogStorageUnavailable,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogMigrationFailed,
                WorkspaceResourceError::WorkspaceCatalogMigrationFailed,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogQueryFailed,
                WorkspaceResourceError::WorkspaceCatalogQueryFailed,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogCorrupt,
                WorkspaceResourceError::WorkspaceCatalogCorrupt,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogSchemaMismatch,
                WorkspaceResourceError::WorkspaceCatalogSchemaMismatch,
            ),
        ] {
            assert_eq!(WorkspaceResourceError::from(setup_error), expected);
        }
    }

    #[test]
    fn runpod_http_initialization_error_maps_to_provider_unavailable() {
        assert_eq!(
            WorkspaceResourceError::from(crate::provider::runpod::RunPodHttpClientInitError),
            WorkspaceResourceError::ProviderApiUnavailable
        );
    }
}
