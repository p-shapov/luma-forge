use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provider::runpod::RunpodProviderError, runtime_catalog::RuntimeCatalogError,
    secrets::SecretsStorageError, shared::ApiError, workflow_catalog::WorkflowCatalogError,
    workspace::WorkspaceError, workspace_catalog::WorkspaceCatalogError,
};

pub type CommandResult<T> = Result<T, NativeCommandError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct NativeCommandError {
    pub message: String,
    pub code: NativeCommandErrorCode,
    pub diagnostic_id: String,
}

impl NativeCommandError {
    pub fn native_initialization(error: NativeInitializationCommandError) -> Self {
        NativeCommandErrorCode::from(error).into()
    }

    pub(crate) fn new(
        code: NativeCommandErrorCode,
        message: impl Into<String>,
        diagnostic_id: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            code,
            diagnostic_id: diagnostic_id.into(),
        }
    }
}

impl From<NativeCommandErrorCode> for NativeCommandError {
    fn from(error: NativeCommandErrorCode) -> Self {
        let message = error.to_string();

        Self {
            message,
            code: error,
            diagnostic_id: crate::diagnostics::new_diagnostic_id(),
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
    #[error("runtime catalog parse failed")]
    RuntimeCatalogParseFailed,
    #[error("runtime catalog validation failed")]
    RuntimeCatalogValidationFailed,
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
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<WorkflowCatalogError> for NativeCommandErrorCode {
    fn from(error: WorkflowCatalogError) -> Self {
        Self::from(&error)
    }
}

impl From<RuntimeCatalogError> for NativeCommandError {
    fn from(error: RuntimeCatalogError) -> Self {
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<RuntimeCatalogError> for NativeCommandErrorCode {
    fn from(error: RuntimeCatalogError) -> Self {
        Self::from(&error)
    }
}

impl From<&RuntimeCatalogError> for NativeCommandErrorCode {
    fn from(error: &RuntimeCatalogError) -> Self {
        match error {
            RuntimeCatalogError::ParseFailed { .. } => Self::RuntimeCatalogParseFailed,
            RuntimeCatalogError::ValidationFailed { .. } => Self::RuntimeCatalogValidationFailed,
        }
    }
}

impl From<&WorkflowCatalogError> for NativeCommandErrorCode {
    fn from(error: &WorkflowCatalogError) -> Self {
        match error {
            WorkflowCatalogError::ParseFailed { .. } => Self::WorkflowCatalogParseFailed,
            WorkflowCatalogError::ValidationFailed { .. } => Self::WorkflowCatalogValidationFailed,
        }
    }
}

impl From<WorkspaceCatalogError> for NativeCommandError {
    fn from(error: WorkspaceCatalogError) -> Self {
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<WorkspaceCatalogError> for NativeCommandErrorCode {
    fn from(error: WorkspaceCatalogError) -> Self {
        Self::from(&error)
    }
}

impl From<&WorkspaceCatalogError> for NativeCommandErrorCode {
    fn from(error: &WorkspaceCatalogError) -> Self {
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
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<SecretsStorageError> for NativeCommandErrorCode {
    fn from(error: SecretsStorageError) -> Self {
        Self::from(&error)
    }
}

impl From<&SecretsStorageError> for NativeCommandErrorCode {
    fn from(error: &SecretsStorageError) -> Self {
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
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<ApiError> for NativeCommandErrorCode {
    fn from(error: ApiError) -> Self {
        Self::from(&error)
    }
}

impl From<&ApiError> for NativeCommandErrorCode {
    fn from(error: &ApiError) -> Self {
        provider_error(error)
    }
}

impl From<&RunpodProviderError> for NativeCommandErrorCode {
    fn from(error: &RunpodProviderError) -> Self {
        match error {
            RunpodProviderError::ProviderApiError(error) => provider_error(error),
            RunpodProviderError::RuntimeProviderApiKeyUnavailable(error) => Self::from(error),
            RunpodProviderError::WorkflowProviderApiKeyUnavailable(error) => Self::from(error),
            RunpodProviderError::ProvisionerWorkerUnavailable { .. }
            | RunpodProviderError::ProvisionerWorkerResponseInvalid { .. }
            | RunpodProviderError::ProvisionerWorkerFailed { .. } => Self::ProviderRequestFailed,
        }
    }
}

impl From<WorkspaceError> for NativeCommandError {
    fn from(error: WorkspaceError) -> Self {
        let message = crate::diagnostics::leaf_error_message(&error);
        let code = NativeCommandErrorCode::from(&error);

        Self::new(code, message, crate::diagnostics::new_diagnostic_id())
    }
}

impl From<WorkspaceError> for NativeCommandErrorCode {
    fn from(error: WorkspaceError) -> Self {
        Self::from(&error)
    }
}

impl From<&WorkspaceError> for NativeCommandErrorCode {
    fn from(error: &WorkspaceError) -> Self {
        match error {
            WorkspaceError::ProviderApiError(error) => provider_error(error),
            WorkspaceError::RuntimeProviderApiKeyUnavailable(error) => Self::from(error),
            WorkspaceError::WorkflowProviderApiKeyUnavailable(error) => Self::from(error),
            WorkspaceError::WorkflowCatalogInvalid(error) => Self::from(error),
            WorkspaceError::RuntimeCatalogInvalid(error) => Self::from(error),
            WorkspaceError::WorkspaceCatalogInvalid(error) => Self::from(error),
            WorkspaceError::LifecycleJournalInvalid { .. } => Self::InvalidRuntimeState,
            WorkspaceError::ProvisionerWorkerUnavailable { .. }
            | WorkspaceError::ProvisionerWorkerResponseInvalid { .. }
            | WorkspaceError::ProvisionerWorkerFailed { .. } => Self::ProviderRequestFailed,
            WorkspaceError::WorkspaceNotFound { .. } => Self::WorkspaceNotFound,
            WorkspaceError::LifecycleOperationAlreadyRunning { .. } => {
                Self::LifecycleOperationAlreadyRunning
            }
            WorkspaceError::InvalidState { .. } => Self::InvalidRuntimeState,
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

fn identity_request_error(error: &ApiError) -> NativeCommandErrorCode {
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

fn provider_error(error: &ApiError) -> NativeCommandErrorCode {
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
        let error = NativeCommandError::new(
            NativeCommandErrorCode::WorkspaceNotFound,
            "workspace was not found",
            "diag-123",
        );

        let json = serde_json::to_string(&error).expect("command error json");

        assert_eq!(
            json,
            r#"{"message":"workspace was not found","code":"workspace_not_found","diagnosticId":"diag-123"}"#
        );
    }

    #[test]
    fn generated_native_command_error_has_diagnostic_id() {
        let error = NativeCommandError::from(WorkspaceCatalogError::WorkspaceNotFound);

        assert_eq!(error.message, "workspace was not found");
        assert_eq!(error.code, NativeCommandErrorCode::WorkspaceNotFound);
        assert!(error.diagnostic_id.starts_with("diag-"));
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
    fn runtime_catalog_error_preserves_service_message() {
        let error = NativeCommandError::from(RuntimeCatalogError::ValidationFailed {
            message: "duplicate runtime contract".to_string(),
        });

        assert_eq!(
            error.message,
            "runtime catalog validation failed: duplicate runtime contract"
        );
        assert_eq!(
            error.code,
            NativeCommandErrorCode::RuntimeCatalogValidationFailed
        );
    }

    #[test]
    fn runpod_unauthorized_preserves_service_message() {
        let error =
            NativeCommandError::from(WorkspaceError::ProviderApiError(ApiError::Unauthorized));

        assert_eq!(error.message, "api request was unauthorized");
        assert_eq!(error.code, NativeCommandErrorCode::ProviderUnauthorized);
    }

    #[test]
    fn wrapped_runtime_error_uses_leaf_source_message() {
        let error = NativeCommandError::from(WorkspaceError::RuntimeProviderApiKeyUnavailable(
            SecretsStorageError::StoreUnavailable,
        ));

        assert_eq!(error.message, "secure storage is unavailable");
        assert_eq!(error.code, NativeCommandErrorCode::StoreUnavailable);
    }

    #[test]
    fn lifecycle_operation_already_running_preserves_workspace_id_in_message() {
        let error = NativeCommandError::from(WorkspaceError::LifecycleOperationAlreadyRunning {
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
        let error = NativeCommandError::from(WorkspaceError::InvalidState {
            message: "runtime state contains provider details".to_string(),
        });

        assert_eq!(
            error.message,
            "invalid runtime state: runtime state contains provider details"
        );
        assert_eq!(error.code, NativeCommandErrorCode::InvalidRuntimeState);
    }
}
