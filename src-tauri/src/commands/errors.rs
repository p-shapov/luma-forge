use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    runpod_runtime::errors::RunpodRuntimeError, secrets_storage::SecretsStorageError,
    shared::ApiError, workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

pub type CommandResult<T> = Result<T, NativeCommandError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct NativeCommandError {
    pub message: String,
    pub code: NativeCommandErrorCode,
}

impl NativeCommandError {
    pub fn native_initialization(error: NativeInitializationCommandError) -> Self {
        NativeCommandErrorCode::from(error).into()
    }

    fn new(code: NativeCommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }
}

impl From<NativeCommandErrorCode> for NativeCommandError {
    fn from(error: NativeCommandErrorCode) -> Self {
        let message = error.to_string();

        Self {
            message,
            code: error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    #[error("workflow catalog parse failed")]
    WorkflowCatalogParseFailed,
    #[error("workflow catalog validation failed")]
    WorkflowCatalogValidationFailed,
    #[error("workspace catalog storage unavailable")]
    WorkspaceCatalogStorageUnavailable,
    #[error("workspace catalog schema is invalid")]
    WorkspaceCatalogSchemaInvalid,
    #[error("workspace catalog data is invalid")]
    WorkspaceCatalogDataInvalid,
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("api key is required")]
    SecretRequired,
    #[error("api key is already configured")]
    KeyAlreadyExists,
    #[error("api key is not configured")]
    KeyNotFound,
    #[error("secure storage is unavailable")]
    StoreUnavailable,
    #[error("stored api key is invalid")]
    StoredSecretInvalid,
    #[error("api key identity request was unauthorized")]
    IdentityUnauthorized,
    #[error("api key identity request has insufficient permissions")]
    IdentityInsufficientPermissions,
    #[error("api key identity request was rate limited")]
    IdentityRateLimited,
    #[error("api key identity request timed out")]
    IdentityTimeout,
    #[error("api key identity request failed")]
    IdentityRequestFailed,
    #[error("api key identity response is invalid")]
    IdentityResponseInvalid,
    #[error("provider request was unauthorized")]
    ProviderUnauthorized,
    #[error("provider request has insufficient permissions")]
    ProviderInsufficientPermissions,
    #[error("provider request was rate limited")]
    ProviderRateLimited,
    #[error("provider request timed out")]
    ProviderTimeout,
    #[error("provider request failed")]
    ProviderRequestFailed,
    #[error("runpod api key unavailable")]
    RunpodApiKeyUnavailable,
    #[error("hugging face api key unavailable")]
    HuggingFaceApiKeyUnavailable,
    #[error("runpod workflow catalog invalid")]
    RunpodWorkflowCatalogInvalid,
    #[error("runpod workspace catalog invalid")]
    RunpodWorkspaceCatalogInvalid,
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable,
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid,
    #[error("provisioner worker failed")]
    ProvisionerWorkerFailed,
    #[error("runpod workspace was not found")]
    RunpodWorkspaceNotFound,
    #[error("workspace already has a running lifecycle operation")]
    LifecycleOperationAlreadyRunning,
    #[error("invalid runtime state")]
    InvalidRuntimeState,
    #[error("app data directory is unavailable")]
    AppDataDirectoryUnavailable,
    #[error("app data directory could not be created")]
    AppDataDirectoryCreateFailed,
    #[error("workspace storage could not be initialized")]
    WorkspaceStorageInitializationFailed,
    #[error("workspace lifecycle state could not be restored")]
    LifecycleStateRestoreFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NativeInitializationCommandError {
    #[error("app data directory is unavailable: {message}")]
    AppDataDirectoryUnavailable { message: String },
    #[error("app data directory could not be created at {path}: {message}")]
    AppDataDirectoryCreateFailed { path: String, message: String },
    #[error("workspace storage could not be initialized at {path}: {message}")]
    WorkspaceStorageInitializationFailed { path: String, message: String },
    #[error("workspace lifecycle state could not be restored: {message}")]
    LifecycleStateRestoreFailed { message: String },
}

impl From<WorkflowCatalogError> for NativeCommandError {
    fn from(error: WorkflowCatalogError) -> Self {
        let message = error.to_string();
        let code = NativeCommandErrorCode::from(error);

        Self::new(code, message)
    }
}

impl From<WorkflowCatalogError> for NativeCommandErrorCode {
    fn from(error: WorkflowCatalogError) -> Self {
        match error {
            WorkflowCatalogError::ParseFailed { .. } => Self::WorkflowCatalogParseFailed,
            WorkflowCatalogError::ValidationFailed { .. } => Self::WorkflowCatalogValidationFailed,
        }
    }
}

impl From<WorkspaceCatalogError> for NativeCommandError {
    fn from(error: WorkspaceCatalogError) -> Self {
        let message = error.to_string();
        let code = NativeCommandErrorCode::from(error);

        Self::new(code, message)
    }
}

impl From<WorkspaceCatalogError> for NativeCommandErrorCode {
    fn from(error: WorkspaceCatalogError) -> Self {
        match error {
            WorkspaceCatalogError::StorageUnavailable { .. } => {
                Self::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceCatalogError::SchemaInvalid { .. } => Self::WorkspaceCatalogSchemaInvalid,
            WorkspaceCatalogError::DataInvalid { .. } => Self::WorkspaceCatalogDataInvalid,
            WorkspaceCatalogError::WorkspaceAlreadyExists => Self::WorkspaceAlreadyExists,
            WorkspaceCatalogError::WorkspaceNotFound => Self::WorkspaceNotFound,
        }
    }
}

impl From<SecretsStorageError> for NativeCommandError {
    fn from(error: SecretsStorageError) -> Self {
        let message = error.to_string();
        let code = NativeCommandErrorCode::from(error);

        Self::new(code, message)
    }
}

impl From<SecretsStorageError> for NativeCommandErrorCode {
    fn from(error: SecretsStorageError) -> Self {
        match error {
            SecretsStorageError::SecretRequired => Self::SecretRequired,
            SecretsStorageError::KeyAlreadyExists => Self::KeyAlreadyExists,
            SecretsStorageError::KeyNotFound => Self::KeyNotFound,
            SecretsStorageError::StoreUnavailable => Self::StoreUnavailable,
            SecretsStorageError::StoredSecretInvalid => Self::StoredSecretInvalid,
            SecretsStorageError::IdentityRequestFailed(error) => identity_request_error(error),
            SecretsStorageError::IdentityResponseInvalid { .. } => Self::IdentityResponseInvalid,
        }
    }
}

impl From<ApiError> for NativeCommandError {
    fn from(error: ApiError) -> Self {
        let message = error.to_string();
        let code = provider_error(error);

        Self::new(code, message)
    }
}

impl From<RunpodRuntimeError> for NativeCommandError {
    fn from(error: RunpodRuntimeError) -> Self {
        let message = error.to_string();
        let code = NativeCommandErrorCode::from(error);

        Self::new(code, message)
    }
}

impl From<RunpodRuntimeError> for NativeCommandErrorCode {
    fn from(error: RunpodRuntimeError) -> Self {
        match error {
            RunpodRuntimeError::RunpodApiError(error) => provider_error(error),
            RunpodRuntimeError::RunpodApiKeyUnavailable(_) => Self::RunpodApiKeyUnavailable,
            RunpodRuntimeError::HuggingFaceApiKeyUnavailable(_) => {
                Self::HuggingFaceApiKeyUnavailable
            }
            RunpodRuntimeError::WorkflowCatalogInvalid(_) => Self::RunpodWorkflowCatalogInvalid,
            RunpodRuntimeError::WorkspaceCatalogInvalid(_) => Self::RunpodWorkspaceCatalogInvalid,
            RunpodRuntimeError::ProvisionerWorkerUnavailable { .. } => {
                Self::ProvisionerWorkerUnavailable
            }
            RunpodRuntimeError::ProvisionerWorkerResponseInvalid { .. } => {
                Self::ProvisionerWorkerResponseInvalid
            }
            RunpodRuntimeError::ProvisionerWorkerFailed { .. } => Self::ProvisionerWorkerFailed,
            RunpodRuntimeError::WorkspaceNotFound { .. } => Self::RunpodWorkspaceNotFound,
            RunpodRuntimeError::LifecycleOperationAlreadyRunning { .. } => {
                Self::LifecycleOperationAlreadyRunning
            }
            RunpodRuntimeError::InvalidRuntimeState { .. } => Self::InvalidRuntimeState,
        }
    }
}

impl From<NativeInitializationCommandError> for NativeCommandErrorCode {
    fn from(error: NativeInitializationCommandError) -> Self {
        match error {
            NativeInitializationCommandError::AppDataDirectoryUnavailable { .. } => {
                Self::AppDataDirectoryUnavailable
            }
            NativeInitializationCommandError::AppDataDirectoryCreateFailed { .. } => {
                Self::AppDataDirectoryCreateFailed
            }
            NativeInitializationCommandError::WorkspaceStorageInitializationFailed { .. } => {
                Self::WorkspaceStorageInitializationFailed
            }
            NativeInitializationCommandError::LifecycleStateRestoreFailed { .. } => {
                Self::LifecycleStateRestoreFailed
            }
        }
    }
}

fn identity_request_error(error: ApiError) -> NativeCommandErrorCode {
    match error {
        ApiError::Unauthorized => NativeCommandErrorCode::IdentityUnauthorized,
        ApiError::InsufficientPermissions => {
            NativeCommandErrorCode::IdentityInsufficientPermissions
        }
        ApiError::RateLimited => NativeCommandErrorCode::IdentityRateLimited,
        ApiError::Timeout => NativeCommandErrorCode::IdentityTimeout,
        ApiError::RequestFailed { .. } => NativeCommandErrorCode::IdentityRequestFailed,
    }
}

fn provider_error(error: ApiError) -> NativeCommandErrorCode {
    match error {
        ApiError::Unauthorized => NativeCommandErrorCode::ProviderUnauthorized,
        ApiError::InsufficientPermissions => {
            NativeCommandErrorCode::ProviderInsufficientPermissions
        }
        ApiError::RateLimited => NativeCommandErrorCode::ProviderRateLimited,
        ApiError::Timeout => NativeCommandErrorCode::ProviderTimeout,
        ApiError::RequestFailed { .. } => NativeCommandErrorCode::ProviderRequestFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_error_serializes_message_and_tagged_error() {
        let error = NativeCommandError::from(WorkspaceCatalogError::WorkspaceNotFound);

        let json = serde_json::to_string(&error).expect("command error json");

        assert_eq!(
            json,
            r#"{"message":"workspace was not found","code":"workspace_not_found"}"#
        );
    }

    #[test]
    fn secrets_store_unavailable_maps_to_exact_error_without_details() {
        let error = NativeCommandError::from(SecretsStorageError::StoreUnavailable);

        assert_eq!(error.message, "secure storage is unavailable");
        assert_eq!(error.code, NativeCommandErrorCode::StoreUnavailable);
    }

    #[test]
    fn workspace_catalog_error_preserves_service_message() {
        let error = NativeCommandError::from(WorkspaceCatalogError::SchemaInvalid {
            message: "missing workspace index".to_string(),
        });

        assert_eq!(
            error.message,
            "workspace catalog schema is invalid: missing workspace index"
        );
        assert_eq!(
            error.code,
            NativeCommandErrorCode::WorkspaceCatalogSchemaInvalid
        );
    }

    #[test]
    fn runpod_unauthorized_preserves_service_message() {
        let error =
            NativeCommandError::from(RunpodRuntimeError::RunpodApiError(ApiError::Unauthorized));

        assert_eq!(error.message, "runpod api error");
        assert_eq!(error.code, NativeCommandErrorCode::ProviderUnauthorized);
    }

    #[test]
    fn lifecycle_operation_already_running_preserves_workspace_id_in_message() {
        let error =
            NativeCommandError::from(RunpodRuntimeError::LifecycleOperationAlreadyRunning {
                workspace_id: "workspace-1".to_string(),
            });

        assert_eq!(
            error.message,
            "workspace already has a running lifecycle operation: workspace-1"
        );
        assert_eq!(
            error.code,
            NativeCommandErrorCode::LifecycleOperationAlreadyRunning
        );
    }

    #[test]
    fn invalid_runtime_state_preserves_service_message() {
        let error = NativeCommandError::from(RunpodRuntimeError::InvalidRuntimeState {
            message: "runtime state contains provider details".to_string(),
        });

        assert_eq!(
            error.message,
            "invalid runtime state: runtime state contains provider details"
        );
        assert_eq!(error.code, NativeCommandErrorCode::InvalidRuntimeState);
    }
}
