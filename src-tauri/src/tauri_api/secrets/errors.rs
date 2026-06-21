use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    secrets::SecretsStorageError,
    tauri_api::{errors::CommandErrorCode, NativeInitializationCommandError},
};

macro_rules! define_secret_error_code {
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
            #[error("api key identity response is invalid")]
            IdentityResponseInvalid,
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
                    "secret_required" => Self::SecretRequired,
                    "key_already_exists" => Self::KeyAlreadyExists,
                    "key_not_found" => Self::KeyNotFound,
                    "store_unavailable" => Self::StoreUnavailable,
                    "stored_secret_invalid" => Self::StoredSecretInvalid,
                    "unauthorized" => Self::Unauthorized,
                    "insufficient_permissions" => Self::InsufficientPermissions,
                    "rate_limited" => Self::RateLimited,
                    "timeout" => Self::Timeout,
                    "request_failed" => Self::RequestFailed,
                    "identity_response_invalid" => Self::IdentityResponseInvalid,
                    _ => Self::CommandError,
                }
            }
        }
    };
}

define_secret_error_code!(SetupRunpodApiKeyErrorCode);
define_secret_error_code!(GetRunpodApiKeyIdentityErrorCode);
define_secret_error_code!(DeleteRunpodApiKeyErrorCode);
define_secret_error_code!(SetupHuggingFaceApiKeyErrorCode);
define_secret_error_code!(GetHuggingFaceApiKeyIdentityErrorCode);
define_secret_error_code!(DeleteHuggingFaceApiKeyErrorCode);

macro_rules! define_secret_command_error {
    ($name:ident, $message:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
        #[serde(untagged)]
        pub(crate) enum $name {
            #[error("native initialization failed")]
            NativeInitialization(#[from] NativeInitializationCommandError),
            #[error($message)]
            SecretsStorage(#[from] SecretsStorageError),
        }
    };
}

define_secret_command_error!(SetupRunpodApiKeyCommandError, "runpod api key setup failed");
define_secret_command_error!(
    GetRunpodApiKeyIdentityCommandError,
    "runpod api key identity failed"
);
define_secret_command_error!(
    DeleteRunpodApiKeyCommandError,
    "runpod api key deletion failed"
);

define_secret_command_error!(
    SetupHuggingFaceApiKeyCommandError,
    "hugging face api key setup failed"
);
define_secret_command_error!(
    GetHuggingFaceApiKeyIdentityCommandError,
    "hugging face api key identity failed"
);
define_secret_command_error!(
    DeleteHuggingFaceApiKeyCommandError,
    "hugging face api key deletion failed"
);
