use serde::{Deserialize, Serialize};
use specta::Type;

use crate::tauri_api::errors::{is_native_initialization_diagnostics_code, CommandErrorCode};

macro_rules! impl_catalog_error_code {
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
pub enum GetWorkflowCatalogErrorCode {
    NativeInitializationFailed,
    ParseFailed,
    ValidationFailed,
    CommandError,
}

impl_catalog_error_code!(GetWorkflowCatalogErrorCode {
    "parse_failed" => ParseFailed,
    "validation_failed" => ValidationFailed,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetRuntimeContractCatalogErrorCode {
    NativeInitializationFailed,
    ParseFailed,
    ValidationFailed,
    CommandError,
}

impl_catalog_error_code!(GetRuntimeContractCatalogErrorCode {
    "parse_failed" => ParseFailed,
    "validation_failed" => ValidationFailed,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetRunpodPlacementOptionsErrorCode {
    NativeInitializationFailed,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    CommandError,
}

impl_catalog_error_code!(GetRunpodPlacementOptionsErrorCode {
    "unauthorized" => Unauthorized,
    "insufficient_permissions" => InsufficientPermissions,
    "rate_limited" => RateLimited,
    "timeout" => Timeout,
    "request_failed" => RequestFailed,
    "key_not_found" => KeyNotFound,
    "store_unavailable" => StoreUnavailable,
    "stored_secret_invalid" => StoredSecretInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetWorkspaceCatalogErrorCode {
    NativeInitializationFailed,
    StorageUnavailable,
    SchemaInvalid,
    DataInvalid,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
    CommandError,
}

impl_catalog_error_code!(GetWorkspaceCatalogErrorCode {
    "storage_unavailable" => StorageUnavailable,
    "schema_invalid" => SchemaInvalid,
    "data_invalid" => DataInvalid,
    "workspace_already_exists" => WorkspaceAlreadyExists,
    "workspace_not_found" => WorkspaceNotFound,
});
