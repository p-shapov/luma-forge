pub mod catalog;
pub mod secrets;
pub mod types;
pub mod workspaces;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::{provider::ProviderApiError, workspace::ProvisionedRemoteComputeProvisioningError},
    provisioned_remote_compute::errors::ProvisionedRemoteComputeError,
    secrets_storage::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

pub type CommandResult<T> = Result<T, NativeCommandError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    WorkflowCatalogInvalid,
    WorkspaceStorageUnavailable,
    WorkspaceStorageQueryFailed,
    WorkspaceStorageCorrupt,
    WorkspaceStorageSchemaMismatch,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
    ProviderUnavailable,
    ProviderSecretUnavailable,
    ProviderUnauthorized,
    ProviderInsufficientPermissions,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed,
    ProvisioningAlreadyRunning,
    InvalidProvisioningState,
    ProvisionerWorkerUnauthorized,
    ProvisionerWorkerUnavailable,
    ProvisionerWorkerConflict,
    ProvisionerWorkerResponseInvalid,
    ProvisionerWorkerFailed,
    CommandNotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
}

impl NativeCommandError {
    pub fn new(code: NativeCommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<WorkflowCatalogError> for NativeCommandError {
    fn from(error: WorkflowCatalogError) -> Self {
        match error {
            WorkflowCatalogError::ParseFailed => Self::new(
                NativeCommandErrorCode::WorkflowCatalogInvalid,
                "workflow catalog could not be read",
            ),
            WorkflowCatalogError::ValidationFailed => Self::new(
                NativeCommandErrorCode::WorkflowCatalogInvalid,
                "workflow catalog is invalid",
            ),
        }
    }
}

impl From<WorkspaceCatalogError> for NativeCommandError {
    fn from(error: WorkspaceCatalogError) -> Self {
        match error {
            WorkspaceCatalogError::StorageUnavailable => Self::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace storage is unavailable",
            ),
            WorkspaceCatalogError::MigrationFailed => Self::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace storage could not be initialized",
            ),
            WorkspaceCatalogError::QueryFailed => Self::new(
                NativeCommandErrorCode::WorkspaceStorageQueryFailed,
                "workspace storage query failed",
            ),
            WorkspaceCatalogError::Corrupt => Self::new(
                NativeCommandErrorCode::WorkspaceStorageCorrupt,
                "workspace storage contains invalid data",
            ),
            WorkspaceCatalogError::SchemaMismatch => Self::new(
                NativeCommandErrorCode::WorkspaceStorageSchemaMismatch,
                "workspace storage schema is incompatible",
            ),
            WorkspaceCatalogError::WorkspaceAlreadyExists => Self::new(
                NativeCommandErrorCode::WorkspaceAlreadyExists,
                "workspace already exists",
            ),
            WorkspaceCatalogError::WorkspaceNotFound => Self::new(
                NativeCommandErrorCode::WorkspaceNotFound,
                "workspace was not found",
            ),
        }
    }
}

impl From<SecretsStorageError> for NativeCommandError {
    fn from(error: SecretsStorageError) -> Self {
        match error {
            SecretsStorageError::SecretRequired => Self::new(
                NativeCommandErrorCode::ProviderSecretUnavailable,
                "api key is required",
            ),
            SecretsStorageError::KeyAlreadyExists => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key is already configured",
            ),
            SecretsStorageError::KeyNotFound => Self::new(
                NativeCommandErrorCode::ProviderSecretUnavailable,
                "api key is not configured",
            ),
            SecretsStorageError::StoreUnavailable => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "secure storage is unavailable",
            ),
            SecretsStorageError::StoredSecretInvalid => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "stored api key is invalid",
            ),
            SecretsStorageError::Provider(_) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key could not be validated",
            ),
            SecretsStorageError::IdentityResponseInvalid => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key identity response is invalid",
            ),
        }
    }
}

impl From<ProvisionedRemoteComputeError> for NativeCommandError {
    fn from(error: ProvisionedRemoteComputeError) -> Self {
        match error {
            ProvisionedRemoteComputeError::SetupWorkspaceInvalidRequest { message } => {
                Self::new(NativeCommandErrorCode::InvalidProvisioningState, message)
            }
            ProvisionedRemoteComputeError::ProviderUnavailable { .. } => Self::new(
                NativeCommandErrorCode::ProviderUnavailable,
                "remote provider is unavailable",
            ),
            ProvisionedRemoteComputeError::ProviderSecretUnavailable => Self::new(
                NativeCommandErrorCode::ProviderSecretUnavailable,
                "api key is not configured",
            ),
            ProvisionedRemoteComputeError::ProvisioningAlreadyRunning { .. } => Self::new(
                NativeCommandErrorCode::ProvisioningAlreadyRunning,
                "workspace provisioning is already running",
            ),
            ProvisionedRemoteComputeError::Provider(ProviderApiError::Unauthorized) => Self::new(
                NativeCommandErrorCode::ProviderUnauthorized,
                "remote provider request failed",
            ),
            ProvisionedRemoteComputeError::Provider(ProviderApiError::InsufficientPermissions) => {
                Self::new(
                    NativeCommandErrorCode::ProviderInsufficientPermissions,
                    "remote provider request failed",
                )
            }
            ProvisionedRemoteComputeError::Provider(ProviderApiError::RateLimited) => Self::new(
                NativeCommandErrorCode::ProviderRateLimited,
                "remote provider request failed",
            ),
            ProvisionedRemoteComputeError::Provider(ProviderApiError::Timeout) => Self::new(
                NativeCommandErrorCode::ProviderTimeout,
                "remote provider request failed",
            ),
            ProvisionedRemoteComputeError::Provider(ProviderApiError::RequestFailed { .. }) => {
                Self::new(
                    NativeCommandErrorCode::ProviderRequestFailed,
                    "remote provider request failed",
                )
            }
            ProvisionedRemoteComputeError::RemoteVolumeNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote volume was not found",
            ),
            ProvisionedRemoteComputeError::RemoteProvisionerNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote provisioner was not found",
            ),
            ProvisionedRemoteComputeError::RemoteEndpointNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote endpoint was not found",
            ),
            ProvisionedRemoteComputeError::ProvisionerWorker(
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnauthorized,
            ) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerUnauthorized,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteComputeError::ProvisionerWorker(
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnavailable,
            ) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerUnavailable,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteComputeError::ProvisionerWorker(
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerConflict,
            ) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerConflict,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteComputeError::ProvisionerWorker(
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerResponseInvalid,
            ) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerResponseInvalid,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteComputeError::ProvisionerWorker(_) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerFailed,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotReady => Self::new(
                NativeCommandErrorCode::InvalidProvisioningState,
                "workspace is not ready",
            ),
            ProvisionedRemoteComputeError::ExecuteWorkspaceMissingEndpoint => Self::new(
                NativeCommandErrorCode::InvalidProvisioningState,
                "workspace endpoint is missing",
            ),
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotImplemented { message } => {
                Self::new(NativeCommandErrorCode::CommandNotImplemented, message)
            }
            ProvisionedRemoteComputeError::DeleteWorkspaceFailed { .. } => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "workspace could not be deleted",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_error_serializes_code_and_message() {
        let error = NativeCommandError::new(
            NativeCommandErrorCode::WorkspaceNotFound,
            "workspace was not found",
        );

        let json = serde_json::to_string(&error).expect("command error json");

        assert_eq!(
            json,
            r#"{"code":"workspace_not_found","message":"workspace was not found"}"#
        );
    }

    #[test]
    fn workspace_storage_not_found_maps_to_stable_code() {
        let error = NativeCommandError::from(WorkspaceCatalogError::WorkspaceNotFound);

        assert_eq!(error.code, NativeCommandErrorCode::WorkspaceNotFound);
        assert_eq!(error.message, "workspace was not found");
    }

    #[test]
    fn secrets_store_unavailable_maps_to_provider_request_failed_without_details() {
        let error = NativeCommandError::from(SecretsStorageError::StoreUnavailable);

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRequestFailed);
        assert_eq!(error.message, "secure storage is unavailable");
    }

    #[test]
    fn provider_unauthorized_maps_to_stable_code_without_provider_details() {
        let error = NativeCommandError::from(ProvisionedRemoteComputeError::Provider(
            ProviderApiError::Unauthorized,
        ));

        assert_eq!(error.code, NativeCommandErrorCode::ProviderUnauthorized);
        assert_eq!(error.message, "remote provider request failed");
    }

    #[test]
    fn provisioner_worker_conflict_maps_to_stable_code_without_worker_details() {
        let error = NativeCommandError::from(ProvisionedRemoteComputeError::ProvisionerWorker(
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerConflict,
        ));

        assert_eq!(
            error.code,
            NativeCommandErrorCode::ProvisionerWorkerConflict
        );
        assert_eq!(error.message, "remote provisioner worker failed");
    }

    #[test]
    fn delete_workspace_failed_uses_fixed_ui_safe_message() {
        let error =
            NativeCommandError::from(ProvisionedRemoteComputeError::DeleteWorkspaceFailed {
                message: "provider leaked detail".to_string(),
            });

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRequestFailed);
        assert_eq!(error.message, "workspace could not be deleted");
    }
}
