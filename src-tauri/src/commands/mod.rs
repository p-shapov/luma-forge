pub mod catalog;
pub mod secrets;
pub mod types;
pub mod workspaces;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::provisioned_remote::ProviderApiError,
    provisioned_remote::errors::ProvisionedRemoteError, secrets_storage::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError, workspace_catalog::WorkspaceCatalogError,
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
    LifecycleOperationAlreadyRunning,
    InvalidRuntimeState,
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

impl From<ProvisionedRemoteError> for NativeCommandError {
    fn from(error: ProvisionedRemoteError) -> Self {
        match error {
            ProvisionedRemoteError::WorkspaceNotFound => Self::new(
                NativeCommandErrorCode::WorkspaceNotFound,
                "workspace was not found",
            ),
            ProvisionedRemoteError::WorkspaceAlreadyExists => Self::new(
                NativeCommandErrorCode::WorkspaceAlreadyExists,
                "workspace already exists",
            ),
            ProvisionedRemoteError::LifecycleOperationAlreadyRunning { .. } => Self::new(
                NativeCommandErrorCode::LifecycleOperationAlreadyRunning,
                "workspace lifecycle operation is already running",
            ),
            ProvisionedRemoteError::ProviderSecretUnavailable => Self::new(
                NativeCommandErrorCode::ProviderSecretUnavailable,
                "api key is not configured",
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::Unauthorized) => Self::new(
                NativeCommandErrorCode::ProviderUnauthorized,
                "remote provider request failed",
            ),
            ProvisionedRemoteError::ProviderApiFailed(
                ProviderApiError::InsufficientPermissions,
            ) => Self::new(
                NativeCommandErrorCode::ProviderInsufficientPermissions,
                "remote provider request failed",
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RateLimited) => Self::new(
                NativeCommandErrorCode::ProviderRateLimited,
                "remote provider request failed",
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::Timeout) => Self::new(
                NativeCommandErrorCode::ProviderTimeout,
                "remote provider request failed",
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RequestFailed) => {
                Self::new(
                    NativeCommandErrorCode::ProviderRequestFailed,
                    "remote provider request failed",
                )
            }
            ProvisionedRemoteError::RemoteVolumeNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote volume was not found",
            ),
            ProvisionedRemoteError::RemoteProvisionerNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote provisioner was not found",
            ),
            ProvisionedRemoteError::RemoteEndpointNotFound => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote endpoint was not found",
            ),
            ProvisionedRemoteError::ProvisionerUnavailable => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerUnavailable,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteError::ProvisionerResponseInvalid => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerResponseInvalid,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteError::ProvisionerFailed => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerFailed,
                "remote provisioner worker failed",
            ),
            ProvisionedRemoteError::InvalidRuntimeState => Self::new(
                NativeCommandErrorCode::InvalidRuntimeState,
                "workspace runtime state is invalid",
            ),
            ProvisionedRemoteError::StorageUnavailable => Self::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace storage is unavailable",
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
    fn lifecycle_command_error_codes_serialize_stable_values() {
        let codes = [
            NativeCommandErrorCode::LifecycleOperationAlreadyRunning,
            NativeCommandErrorCode::InvalidRuntimeState,
        ];

        let json = serde_json::to_value(codes).expect("command error code json");

        assert_eq!(
            json,
            serde_json::json!([
                "lifecycle_operation_already_running",
                "invalid_runtime_state"
            ])
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
        let error = NativeCommandError::from(ProvisionedRemoteError::ProviderApiFailed(
            ProviderApiError::Unauthorized,
        ));

        assert_eq!(error.code, NativeCommandErrorCode::ProviderUnauthorized);
        assert_eq!(error.message, "remote provider request failed");
    }

    #[test]
    fn provisioner_worker_conflict_maps_to_stable_code_without_worker_details() {
        let error = NativeCommandError::from(ProvisionedRemoteError::ProvisionerFailed);

        assert_eq!(error.code, NativeCommandErrorCode::ProvisionerWorkerFailed);
        assert_eq!(error.message, "remote provisioner worker failed");
    }

    #[test]
    fn delete_workspace_failed_uses_fixed_ui_safe_message() {
        let error = NativeCommandError::from(ProvisionedRemoteError::InvalidRuntimeState);

        assert_eq!(error.code, NativeCommandErrorCode::InvalidRuntimeState);
        assert_eq!(error.message, "workspace runtime state is invalid");
    }
}
