pub mod catalog;
pub mod secrets;
pub mod types;
pub mod workspaces;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::runpod::{RunpodLifecycleError, RunpodProvisionerError, RunpodRuntimeStateError},
    runpod_runtime::errors::RunpodRuntimeError,
    secrets_storage::SecretsStorageError,
    shared::ApiError,
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
    RunpodSecretUnavailable,
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
            WorkflowCatalogError::ParseFailed { .. } => Self::new(
                NativeCommandErrorCode::WorkflowCatalogInvalid,
                "workflow catalog could not be read",
            ),
            WorkflowCatalogError::ValidationFailed { .. } => Self::new(
                NativeCommandErrorCode::WorkflowCatalogInvalid,
                "workflow catalog is invalid",
            ),
        }
    }
}

impl From<WorkspaceCatalogError> for NativeCommandError {
    fn from(error: WorkspaceCatalogError) -> Self {
        match error {
            WorkspaceCatalogError::StorageUnavailable { .. } => Self::new(
                NativeCommandErrorCode::WorkspaceStorageUnavailable,
                "workspace storage is unavailable",
            ),
            WorkspaceCatalogError::DataInvalid { .. } => Self::new(
                NativeCommandErrorCode::WorkspaceStorageCorrupt,
                "workspace storage contains invalid data",
            ),
            WorkspaceCatalogError::SchemaInvalid { .. } => Self::new(
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
                NativeCommandErrorCode::RunpodSecretUnavailable,
                "api key is required",
            ),
            SecretsStorageError::KeyAlreadyExists => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key is already configured",
            ),
            SecretsStorageError::KeyNotFound => Self::new(
                NativeCommandErrorCode::RunpodSecretUnavailable,
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
            SecretsStorageError::IdentityRequestFailed(_) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key could not be validated",
            ),
            SecretsStorageError::IdentityResponseInvalid { .. } => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "api key identity response is invalid",
            ),
        }
    }
}

impl From<RunpodRuntimeError> for NativeCommandError {
    fn from(error: RunpodRuntimeError) -> Self {
        match error {
            RunpodRuntimeError::RunpodApiKeyUnavailable(_) => Self::new(
                NativeCommandErrorCode::RunpodSecretUnavailable,
                "api key is not configured",
            ),
            RunpodRuntimeError::HuggingFaceApiKeyUnavailable(_) => Self::new(
                NativeCommandErrorCode::RunpodSecretUnavailable,
                "hugging face api key is not configured",
            ),
            RunpodRuntimeError::WorkflowCatalogInvalid(error) => Self::from(error),
            RunpodRuntimeError::WorkspaceCatalogInvalid(error) => Self::from(error),
            RunpodRuntimeError::RunpodApiError(ApiError::Unauthorized)
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::RunPodApiError(
                ApiError::Unauthorized,
            )) => Self::new(
                NativeCommandErrorCode::ProviderUnauthorized,
                "runpod request failed",
            ),
            RunpodRuntimeError::RunpodApiError(ApiError::InsufficientPermissions)
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::RunPodApiError(
                ApiError::InsufficientPermissions,
            )) => Self::new(
                NativeCommandErrorCode::ProviderInsufficientPermissions,
                "runpod request failed",
            ),
            RunpodRuntimeError::RunpodApiError(ApiError::RateLimited)
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::RunPodApiError(
                ApiError::RateLimited,
            )) => Self::new(
                NativeCommandErrorCode::ProviderRateLimited,
                "runpod request failed",
            ),
            RunpodRuntimeError::RunpodApiError(ApiError::Timeout)
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::RunPodApiError(
                ApiError::Timeout,
            )) => Self::new(
                NativeCommandErrorCode::ProviderTimeout,
                "runpod request failed",
            ),
            RunpodRuntimeError::RunpodApiError(ApiError::RequestFailed { .. })
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::RunPodApiError(
                ApiError::RequestFailed { .. },
            )) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "runpod request failed",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingVolume,
            )) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote volume was not found",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingProvisionerPod,
            )) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote provisioner was not found",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingEndpoint,
            )) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote endpoint was not found",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingTemplate,
            )) => Self::new(
                NativeCommandErrorCode::ProviderRequestFailed,
                "remote template was not found",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::ProvisionerError(
                RunpodProvisionerError::Unavailable { .. },
            )) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerUnavailable,
                "remote provisioner worker failed",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::ProvisionerError(
                RunpodProvisionerError::ResponseInvalid { .. },
            )) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerResponseInvalid,
                "remote provisioner worker failed",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::ProvisionerError(
                RunpodProvisionerError::Failed { .. },
            )) => Self::new(
                NativeCommandErrorCode::ProvisionerWorkerFailed,
                "remote provisioner worker failed",
            ),
            RunpodRuntimeError::LifecycleError(RunpodLifecycleError::AppInterrupted)
            | RunpodRuntimeError::LifecycleError(RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::Invalid { .. },
            ))
            | RunpodRuntimeError::InvalidRuntimeState { .. } => Self::new(
                NativeCommandErrorCode::InvalidRuntimeState,
                "workspace runtime state is invalid",
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
    fn runpod_unauthorized_maps_to_stable_code_without_provider_details() {
        let error = NativeCommandError::from(RunpodRuntimeError::from(
            RunpodLifecycleError::RunPodApiError(ApiError::Unauthorized),
        ));

        assert_eq!(error.code, NativeCommandErrorCode::ProviderUnauthorized);
        assert_eq!(error.message, "runpod request failed");
    }

    #[test]
    fn provisioner_worker_conflict_maps_to_stable_code_without_worker_details() {
        let error = NativeCommandError::from(RunpodRuntimeError::from(
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Failed {
                message: "provisioner worker failed".to_string(),
            }),
        ));

        assert_eq!(error.code, NativeCommandErrorCode::ProvisionerWorkerFailed);
        assert_eq!(error.message, "remote provisioner worker failed");
    }

    #[test]
    fn delete_workspace_failed_uses_fixed_ui_safe_message() {
        let error = NativeCommandError::from(
            crate::runpod_runtime::errors::invalid_runtime_state_message(
                "runtime state is invalid",
            ),
        );

        assert_eq!(error.code, NativeCommandErrorCode::InvalidRuntimeState);
        assert_eq!(error.message, "workspace runtime state is invalid");
    }
}
