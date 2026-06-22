use serde::{Deserialize, Serialize};
use specta::Type;

use crate::tauri_api::errors::{is_native_initialization_diagnostics_code, CommandErrorCode};

macro_rules! impl_workspace_error_code {
    ($name:ident { $($diagnostic_code:literal => $code_variant:ident),+ $(,)? }) => {
        impl CommandErrorCode for $name {
            fn from_diagnostics_code(code: &str) -> Self {
                match code {
                    code if is_native_initialization_diagnostics_code(code) => Self::NativeInitializationFailed,
                    $($diagnostic_code => Self::$code_variant,)+
                    _ => Self::CommandError,
                }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CreateRunpodWorkspaceErrorCode {
    NativeInitializationFailed,
    ParseFailed,
    ValidationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    WorkspaceAlreadyExists,
    InvalidState,
    CommandError,
}

impl_workspace_error_code!(CreateRunpodWorkspaceErrorCode {
    "parse_failed" => ParseFailed,
    "validation_failed" => ValidationFailed,
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
    "workspace_already_exists" => WorkspaceAlreadyExists,
    "invalid_state" => InvalidState,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionWorkspaceErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    WorkspaceNotFound,
    LifecycleOperationAlreadyRunning,
    InvalidState,
    CommandError,
}

impl_workspace_error_code!(ProvisionWorkspaceErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
    "workspace_not_found" => WorkspaceNotFound,
    "lifecycle_operation_already_running" => LifecycleOperationAlreadyRunning,
    "invalid_state" => InvalidState,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupWorkspaceErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    WorkspaceNotFound,
    LifecycleOperationAlreadyRunning,
    InvalidState,
    CommandError,
}

impl_workspace_error_code!(CleanupWorkspaceErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
    "workspace_not_found" => WorkspaceNotFound,
    "lifecycle_operation_already_running" => LifecycleOperationAlreadyRunning,
    "invalid_state" => InvalidState,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DeleteWorkspaceErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    WorkspaceNotFound,
    LifecycleOperationAlreadyRunning,
    InvalidState,
    CommandError,
}

impl_workspace_error_code!(DeleteWorkspaceErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
    "workspace_not_found" => WorkspaceNotFound,
    "lifecycle_operation_already_running" => LifecycleOperationAlreadyRunning,
    "invalid_state" => InvalidState,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetRunningLifecycleOperationsErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    CommandError,
}

impl_workspace_error_code!(GetRunningLifecycleOperationsErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetLatestLifecycleOperationErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    CommandError,
}

impl_workspace_error_code!(GetLatestLifecycleOperationErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
});
