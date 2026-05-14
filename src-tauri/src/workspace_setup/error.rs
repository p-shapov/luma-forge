use thiserror::Error;

use crate::domain::placement::validator::PlacementValidationError;
use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("stored provider api key invalid")]
    StoredProviderApiKeyInvalid,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider inventory invalid")]
    ProviderInventoryInvalid,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("workflow catalog unavailable")]
    WorkflowCatalogUnavailable,
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
    #[error("placement provider mismatch")]
    PlacementProviderMismatch,
    #[error("placement datacenter required")]
    PlacementDatacenterRequired,
    #[error("placement gpu required")]
    PlacementGpuRequired,
    #[error("workflow preset stale")]
    WorkflowPresetStale,
    #[error("storage size below preset minimum")]
    StorageSizeBelowPresetMinimum,
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("invalid workspace id")]
    InvalidWorkspaceId,
    #[error("workspace name required")]
    WorkspaceNameRequired,
    #[error("invalid workspace metadata")]
    InvalidWorkspaceMetadata,
}

impl From<SecretStoreError> for WorkspaceSetupError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::StoredProviderApiKeyInvalid,
        }
    }
}

impl From<PlacementValidationError> for WorkspaceSetupError {
    fn from(error: PlacementValidationError) -> Self {
        match error {
            PlacementValidationError::ProviderMismatch => Self::PlacementProviderMismatch,
            PlacementValidationError::DatacenterRequired => Self::PlacementDatacenterRequired,
            PlacementValidationError::GpuRequired => Self::PlacementGpuRequired,
            PlacementValidationError::WorkflowPresetStale => Self::WorkflowPresetStale,
            PlacementValidationError::StorageSizeBelowPresetMinimum => {
                Self::StorageSizeBelowPresetMinimum
            }
        }
    }
}
