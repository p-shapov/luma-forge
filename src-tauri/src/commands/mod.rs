pub mod catalog;
pub mod secrets;
pub mod types;
pub mod workspaces;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    remote_workspace::errors::RemoteWorkspaceError, secrets_storage::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError, workspace_catalog::WorkspaceCatalogError,
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

impl From<RemoteWorkspaceError> for NativeCommandError {
    fn from(error: RemoteWorkspaceError) -> Self {
        match error {
            RemoteWorkspaceError::SetupWorkspaceInvalidRequest { message } => Self::new(message),
            RemoteWorkspaceError::ProviderUnavailable { .. } => {
                Self::new("remote provider is unavailable")
            }
            RemoteWorkspaceError::ProviderSecretUnavailable => {
                Self::new("api key is not configured")
            }
            RemoteWorkspaceError::ProvisioningAlreadyRunning { .. } => {
                Self::new("workspace provisioning is already running")
            }
            RemoteWorkspaceError::Provider(_) => Self::new("remote provider request failed"),
            RemoteWorkspaceError::RemoteVolumeNotFound => Self::new("remote volume was not found"),
            RemoteWorkspaceError::RemoteProvisionerNotFound => {
                Self::new("remote provisioner was not found")
            }
            RemoteWorkspaceError::RemoteEndpointNotFound => {
                Self::new("remote endpoint was not found")
            }
            RemoteWorkspaceError::ProvisionerWorker(_) => {
                Self::new("remote provisioner worker failed")
            }
            RemoteWorkspaceError::ExecuteWorkspaceNotReady => Self::new("workspace is not ready"),
            RemoteWorkspaceError::ExecuteWorkspaceMissingEndpoint => {
                Self::new("workspace endpoint is missing")
            }
            RemoteWorkspaceError::ExecuteWorkspaceNotImplemented { message } => Self::new(message),
            RemoteWorkspaceError::DeleteWorkspaceFailed { message } => Self::new(message),
        }
    }
}
