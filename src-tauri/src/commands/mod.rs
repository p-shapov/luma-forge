pub mod catalog;
pub mod secrets;
pub mod types;
pub mod workspaces;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provisioned_remote_compute::errors::ProvisionedRemoteComputeError,
    secrets_storage::SecretsStorageError, workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

pub type CommandResult<T> = Result<T, NativeCommandError>;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NativeCommandError {
    pub message: String,
}

impl NativeCommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<WorkflowCatalogError> for NativeCommandError {
    fn from(error: WorkflowCatalogError) -> Self {
        match error {
            WorkflowCatalogError::ParseFailed => Self::new("workflow catalog could not be read"),
            WorkflowCatalogError::ValidationFailed => Self::new("workflow catalog is invalid"),
        }
    }
}

impl From<WorkspaceCatalogError> for NativeCommandError {
    fn from(error: WorkspaceCatalogError) -> Self {
        match error {
            WorkspaceCatalogError::StorageUnavailable => {
                Self::new("workspace storage is unavailable")
            }
            WorkspaceCatalogError::MigrationFailed => {
                Self::new("workspace storage could not be initialized")
            }
            WorkspaceCatalogError::QueryFailed => Self::new("workspace storage query failed"),
            WorkspaceCatalogError::Corrupt => Self::new("workspace storage contains invalid data"),
            WorkspaceCatalogError::SchemaMismatch => {
                Self::new("workspace storage schema is incompatible")
            }
            WorkspaceCatalogError::WorkspaceAlreadyExists => Self::new("workspace already exists"),
            WorkspaceCatalogError::WorkspaceNotFound => Self::new("workspace was not found"),
        }
    }
}

impl From<SecretsStorageError> for NativeCommandError {
    fn from(error: SecretsStorageError) -> Self {
        match error {
            SecretsStorageError::SecretRequired => Self::new("api key is required"),
            SecretsStorageError::KeyAlreadyExists => Self::new("api key is already configured"),
            SecretsStorageError::KeyNotFound => Self::new("api key is not configured"),
            SecretsStorageError::StoreUnavailable => Self::new("secure storage is unavailable"),
            SecretsStorageError::StoredSecretInvalid => Self::new("stored api key is invalid"),
            SecretsStorageError::Provider(_) => Self::new("api key could not be validated"),
            SecretsStorageError::IdentityResponseInvalid => {
                Self::new("api key identity response is invalid")
            }
        }
    }
}

impl From<ProvisionedRemoteComputeError> for NativeCommandError {
    fn from(error: ProvisionedRemoteComputeError) -> Self {
        match error {
            ProvisionedRemoteComputeError::SetupWorkspaceInvalidRequest { message } => {
                Self::new(message)
            }
            ProvisionedRemoteComputeError::ProviderUnavailable { .. } => {
                Self::new("remote provider is unavailable")
            }
            ProvisionedRemoteComputeError::ProviderSecretUnavailable => {
                Self::new("api key is not configured")
            }
            ProvisionedRemoteComputeError::ProvisioningAlreadyRunning { .. } => {
                Self::new("workspace provisioning is already running")
            }
            ProvisionedRemoteComputeError::Provider(_) => {
                Self::new("remote provider request failed")
            }
            ProvisionedRemoteComputeError::RemoteVolumeNotFound => {
                Self::new("remote volume was not found")
            }
            ProvisionedRemoteComputeError::RemoteProvisionerNotFound => {
                Self::new("remote provisioner was not found")
            }
            ProvisionedRemoteComputeError::RemoteEndpointNotFound => {
                Self::new("remote endpoint was not found")
            }
            ProvisionedRemoteComputeError::ProvisionerWorker(_) => {
                Self::new("remote provisioner worker failed")
            }
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotReady => {
                Self::new("workspace is not ready")
            }
            ProvisionedRemoteComputeError::ExecuteWorkspaceMissingEndpoint => {
                Self::new("workspace endpoint is missing")
            }
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotImplemented { message } => {
                Self::new(message)
            }
            ProvisionedRemoteComputeError::DeleteWorkspaceFailed { message } => Self::new(message),
        }
    }
}
