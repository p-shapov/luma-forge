use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provider::runpod::RunpodProviderError,
    runtime_catalog::RuntimeCatalogError,
    tauri_api::{errors::CommandErrorCode, NativeInitializationCommandError},
    workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

macro_rules! define_catalog_code {
    ($name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
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
            $(
                #[error($code)]
                $variant,
            )+
            #[error("command error")]
            CommandError,
        }

        impl CommandErrorCode for $name {
            fn from_diagnostics_code(code: &str) -> Self {
                match code {
                    "app_data_directory_unavailable" => Self::AppDataDirectoryUnavailable,
                    "app_data_directory_create_failed" => Self::AppDataDirectoryCreateFailed,
                    "diagnostics_initialization_failed" => Self::DiagnosticsInitializationFailed,
                    "workspace_storage_initialization_failed" => Self::WorkspaceStorageInitializationFailed,
                    "provider_services_initialization_failed" => Self::ProviderServicesInitializationFailed,
                    "lifecycle_state_restore_failed" => Self::LifecycleStateRestoreFailed,
                    $($code => Self::$variant,)+
                    _ => Self::CommandError,
                }
            }

        }
    };
}

define_catalog_code!(GetWorkflowCatalogErrorCode {
    ParseFailed => "parse_failed",
    ValidationFailed => "validation_failed",
});

define_catalog_code!(GetRuntimeContractCatalogErrorCode {
    ParseFailed => "parse_failed",
    ValidationFailed => "validation_failed",
});

define_catalog_code!(GetRunpodPlacementOptionsErrorCode {
    Unauthorized => "unauthorized",
    InsufficientPermissions => "insufficient_permissions",
    RateLimited => "rate_limited",
    Timeout => "timeout",
    RequestFailed => "request_failed",
    SecretRequired => "secret_required",
    KeyAlreadyExists => "key_already_exists",
    KeyNotFound => "key_not_found",
    StoreUnavailable => "store_unavailable",
    StoredSecretInvalid => "stored_secret_invalid",
    IdentityResponseInvalid => "identity_response_invalid",
    ProvisionerWorkerUnavailable => "provisioner_worker_unavailable",
    ProvisionerWorkerResponseInvalid => "provisioner_worker_response_invalid",
    ProvisionerWorkerFailed => "provisioner_worker_failed",
});

define_catalog_code!(GetWorkspaceCatalogErrorCode {
    StorageUnavailable => "storage_unavailable",
    SchemaInvalid => "schema_invalid",
    DataInvalid => "data_invalid",
    WorkspaceAlreadyExists => "workspace_already_exists",
    WorkspaceNotFound => "workspace_not_found",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub(crate) enum GetWorkflowCatalogCommandError {
    #[error("native initialization failed")]
    NativeInitialization(#[from] NativeInitializationCommandError),
    #[error("workflow catalog failed")]
    WorkflowCatalog(#[from] WorkflowCatalogError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub(crate) enum GetRuntimeContractCatalogCommandError {
    #[error("native initialization failed")]
    NativeInitialization(#[from] NativeInitializationCommandError),
    #[error("runtime contract catalog failed")]
    RuntimeCatalog(#[from] RuntimeCatalogError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub(crate) enum GetRunpodPlacementOptionsCommandError {
    #[error("native initialization failed")]
    NativeInitialization(#[from] NativeInitializationCommandError),
    #[error("runpod placement options failed")]
    RunpodProvider(#[from] RunpodProviderError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(untagged)]
pub(crate) enum GetWorkspaceCatalogCommandError {
    #[error("native initialization failed")]
    NativeInitialization(#[from] NativeInitializationCommandError),
    #[error("workspace catalog failed")]
    WorkspaceCatalog(#[from] WorkspaceCatalogError),
}
