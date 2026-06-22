use serde::{Deserialize, Serialize};
use specta::Type;

use crate::tauri_api::errors::{is_native_initialization_diagnostics_code, CommandErrorCode};

macro_rules! impl_secret_error_code {
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
pub enum SetupRunpodApiKeyErrorCode {
    NativeInitializationFailed,
    SecretRequired,
    KeyAlreadyExists,
    StoreUnavailable,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
    IdentityResponseInvalid,
    CommandError,
}

impl_secret_error_code!(SetupRunpodApiKeyErrorCode {
    "secret_required" => SecretRequired,
    "key_already_exists" => KeyAlreadyExists,
    "store_unavailable" => StoreUnavailable,
    "unauthorized" => Unauthorized,
    "insufficient_permissions" => InsufficientPermissions,
    "rate_limited" => RateLimited,
    "timeout" => Timeout,
    "request_failed" => RequestFailed,
    "identity_response_invalid" => IdentityResponseInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetRunpodApiKeyIdentityErrorCode {
    NativeInitializationFailed,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
    IdentityResponseInvalid,
    CommandError,
}

impl_secret_error_code!(GetRunpodApiKeyIdentityErrorCode {
    "key_not_found" => KeyNotFound,
    "store_unavailable" => StoreUnavailable,
    "stored_secret_invalid" => StoredSecretInvalid,
    "unauthorized" => Unauthorized,
    "insufficient_permissions" => InsufficientPermissions,
    "rate_limited" => RateLimited,
    "timeout" => Timeout,
    "request_failed" => RequestFailed,
    "identity_response_invalid" => IdentityResponseInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DeleteRunpodApiKeyErrorCode {
    NativeInitializationFailed,
    KeyNotFound,
    StoreUnavailable,
    CommandError,
}

impl_secret_error_code!(DeleteRunpodApiKeyErrorCode {
    "key_not_found" => KeyNotFound,
    "store_unavailable" => StoreUnavailable,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SetupHuggingFaceApiKeyErrorCode {
    NativeInitializationFailed,
    SecretRequired,
    KeyAlreadyExists,
    StoreUnavailable,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
    IdentityResponseInvalid,
    CommandError,
}

impl_secret_error_code!(SetupHuggingFaceApiKeyErrorCode {
    "secret_required" => SecretRequired,
    "key_already_exists" => KeyAlreadyExists,
    "store_unavailable" => StoreUnavailable,
    "unauthorized" => Unauthorized,
    "insufficient_permissions" => InsufficientPermissions,
    "rate_limited" => RateLimited,
    "timeout" => Timeout,
    "request_failed" => RequestFailed,
    "identity_response_invalid" => IdentityResponseInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GetHuggingFaceApiKeyIdentityErrorCode {
    NativeInitializationFailed,
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
    IdentityResponseInvalid,
    CommandError,
}

impl_secret_error_code!(GetHuggingFaceApiKeyIdentityErrorCode {
    "key_not_found" => KeyNotFound,
    "store_unavailable" => StoreUnavailable,
    "stored_secret_invalid" => StoredSecretInvalid,
    "unauthorized" => Unauthorized,
    "insufficient_permissions" => InsufficientPermissions,
    "rate_limited" => RateLimited,
    "timeout" => Timeout,
    "request_failed" => RequestFailed,
    "identity_response_invalid" => IdentityResponseInvalid,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DeleteHuggingFaceApiKeyErrorCode {
    NativeInitializationFailed,
    KeyNotFound,
    StoreUnavailable,
    CommandError,
}

impl_secret_error_code!(DeleteHuggingFaceApiKeyErrorCode {
    "key_not_found" => KeyNotFound,
    "store_unavailable" => StoreUnavailable,
});
