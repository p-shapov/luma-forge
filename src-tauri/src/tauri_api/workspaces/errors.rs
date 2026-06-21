use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    tauri_api::{errors::CommandErrorCode, NativeInitializationCommandError},
    workspace::WorkspaceError,
};

macro_rules! define_workspace_error_code {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[error("app data directory is unavailable")]
            AppDataDirectoryUnavailable,
            #[error("app data directory could not be created")]
            AppDataDirectoryCreateFailed,
            #[error("native diagnostics could not be initialized")]
            DiagnosticsInitializationFailed,
            #[error("workspace storage could not be initialized")]
            WorkspaceStorageInitializationFailed,
            #[error("provider services could not be initialized")]
            ProviderServicesInitializationFailed,
            #[error("workspace lifecycle state could not be restored")]
            LifecycleStateRestoreFailed,
            #[error("api request was unauthorized")]
            Unauthorized,
            #[error("api request has insufficient permissions")]
            InsufficientPermissions,
            #[error("api request was rate limited")]
            RateLimited,
            #[error("api request timed out")]
            Timeout,
            #[error("api request failed")]
            RequestFailed,
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
            #[error("api key identity response is invalid")]
            IdentityResponseInvalid,
            #[error("storage is unavailable")]
            StorageUnavailable,
            #[error("catalog parse failed")]
            ParseFailed,
            #[error("catalog validation failed")]
            ValidationFailed,
            #[error("schema is invalid")]
            SchemaInvalid,
            #[error("data is invalid")]
            DataInvalid,
            #[error("workspace already exists")]
            WorkspaceAlreadyExists,
            #[error("workspace was not found")]
            WorkspaceNotFound,
            #[error("operation was not found")]
            OperationNotFound,
            #[error("running operation exists")]
            RunningOperationExists,
            #[error("workspace already has a running lifecycle operation")]
            LifecycleOperationAlreadyRunning,
            #[error("invalid runtime state")]
            InvalidState,
            #[error("command error")]
            CommandError,
        }

        impl CommandErrorCode for $name {
            fn from_diagnostics_code(code: &str) -> Self {
                match code {
                    "app_data_directory_unavailable" => Self::AppDataDirectoryUnavailable,
                    "app_data_directory_create_failed" => Self::AppDataDirectoryCreateFailed,
                    "diagnostics_initialization_failed" => Self::DiagnosticsInitializationFailed,
                    "workspace_storage_initialization_failed" => {
                        Self::WorkspaceStorageInitializationFailed
                    }
                    "provider_services_initialization_failed" => {
                        Self::ProviderServicesInitializationFailed
                    }
                    "lifecycle_state_restore_failed" => Self::LifecycleStateRestoreFailed,
                    "unauthorized" => Self::Unauthorized,
                    "insufficient_permissions" => Self::InsufficientPermissions,
                    "rate_limited" => Self::RateLimited,
                    "timeout" => Self::Timeout,
                    "request_failed" => Self::RequestFailed,
                    "secret_required" => Self::SecretRequired,
                    "key_already_exists" => Self::KeyAlreadyExists,
                    "key_not_found" => Self::KeyNotFound,
                    "store_unavailable" => Self::StoreUnavailable,
                    "stored_secret_invalid" => Self::StoredSecretInvalid,
                    "identity_response_invalid" => Self::IdentityResponseInvalid,
                    "storage_unavailable" => Self::StorageUnavailable,
                    "parse_failed" => Self::ParseFailed,
                    "validation_failed" => Self::ValidationFailed,
                    "schema_invalid" => Self::SchemaInvalid,
                    "data_invalid" => Self::DataInvalid,
                    "workspace_already_exists" => Self::WorkspaceAlreadyExists,
                    "workspace_not_found" => Self::WorkspaceNotFound,
                    "operation_not_found" => Self::OperationNotFound,
                    "running_operation_exists" => Self::RunningOperationExists,
                    "lifecycle_operation_already_running" => Self::LifecycleOperationAlreadyRunning,
                    "invalid_state" => Self::InvalidState,
                    _ => Self::CommandError,
                }
            }

            fn as_str(&self) -> &'static str {
                match self {
                    Self::AppDataDirectoryUnavailable => "app_data_directory_unavailable",
                    Self::AppDataDirectoryCreateFailed => "app_data_directory_create_failed",
                    Self::DiagnosticsInitializationFailed => "diagnostics_initialization_failed",
                    Self::WorkspaceStorageInitializationFailed => {
                        "workspace_storage_initialization_failed"
                    }
                    Self::ProviderServicesInitializationFailed => {
                        "provider_services_initialization_failed"
                    }
                    Self::LifecycleStateRestoreFailed => "lifecycle_state_restore_failed",
                    Self::Unauthorized => "unauthorized",
                    Self::InsufficientPermissions => "insufficient_permissions",
                    Self::RateLimited => "rate_limited",
                    Self::Timeout => "timeout",
                    Self::RequestFailed => "request_failed",
                    Self::SecretRequired => "secret_required",
                    Self::KeyAlreadyExists => "key_already_exists",
                    Self::KeyNotFound => "key_not_found",
                    Self::StoreUnavailable => "store_unavailable",
                    Self::StoredSecretInvalid => "stored_secret_invalid",
                    Self::IdentityResponseInvalid => "identity_response_invalid",
                    Self::StorageUnavailable => "storage_unavailable",
                    Self::ParseFailed => "parse_failed",
                    Self::ValidationFailed => "validation_failed",
                    Self::SchemaInvalid => "schema_invalid",
                    Self::DataInvalid => "data_invalid",
                    Self::WorkspaceAlreadyExists => "workspace_already_exists",
                    Self::WorkspaceNotFound => "workspace_not_found",
                    Self::OperationNotFound => "operation_not_found",
                    Self::RunningOperationExists => "running_operation_exists",
                    Self::LifecycleOperationAlreadyRunning => "lifecycle_operation_already_running",
                    Self::InvalidState => "invalid_state",
                    Self::CommandError => "command_error",
                }
            }
        }
    };
}

define_workspace_error_code!(CreateRunpodWorkspaceErrorCode);
define_workspace_error_code!(ProvisionWorkspaceErrorCode);
define_workspace_error_code!(CleanupWorkspaceErrorCode);
define_workspace_error_code!(DeleteWorkspaceErrorCode);
define_workspace_error_code!(GetRunningLifecycleOperationsErrorCode);
define_workspace_error_code!(GetLatestLifecycleOperationErrorCode);

macro_rules! define_workspace_command_error {
    ($name:ident, $message:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
        #[serde(untagged)]
        pub(crate) enum $name {
            #[error("native initialization failed: {0}")]
            NativeInitialization(#[from] NativeInitializationCommandError),
            #[error($message)]
            Workspace(#[from] WorkspaceError),
        }
    };
}

define_workspace_command_error!(
    CreateRunpodWorkspaceCommandError,
    "runpod workspace creation failed: {0}"
);
define_workspace_command_error!(
    ProvisionWorkspaceCommandError,
    "workspace provision failed: {0}"
);
define_workspace_command_error!(
    CleanupWorkspaceCommandError,
    "workspace cleanup failed: {0}"
);
define_workspace_command_error!(
    DeleteWorkspaceCommandError,
    "workspace deletion failed: {0}"
);
define_workspace_command_error!(
    GetRunningLifecycleOperationsCommandError,
    "running lifecycle operations lookup failed: {0}"
);
define_workspace_command_error!(
    GetLatestLifecycleOperationCommandError,
    "latest lifecycle operation lookup failed: {0}"
);
