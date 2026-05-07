use serde::Serialize;
use specta::Type;

use super::provider_setup::ProviderSetupError;

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NativeCommandErrorCode {
    UnsupportedProvider,
    ProviderSetupIncomplete,
    ProviderSetupAlreadyExists,
    InvalidProviderApiKey,
    ProviderApiUnavailable,
    SecureKeyringUnavailable,
    LocalStorageUnavailable,
    #[allow(dead_code)]
    InvalidRequest,
}

#[derive(Clone, Debug, Serialize, Type)]
pub(crate) struct NativeCommandError {
    pub(crate) code: NativeCommandErrorCode,
    pub(crate) message: String,
    pub(crate) retryable: bool,
}

impl NativeCommandError {
    fn new(code: NativeCommandErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

impl From<ProviderSetupError> for NativeCommandError {
    fn from(error: ProviderSetupError) -> Self {
        match error {
            ProviderSetupError::UnsupportedProvider => Self::new(
                NativeCommandErrorCode::UnsupportedProvider,
                "Unsupported GPU Cloud Provider.",
                false,
            ),
            ProviderSetupError::ProviderSetupIncomplete => Self::new(
                NativeCommandErrorCode::ProviderSetupIncomplete,
                "GPU Cloud Provider Setup is incomplete.",
                false,
            ),
            ProviderSetupError::ProviderSetupAlreadyExists => Self::new(
                NativeCommandErrorCode::ProviderSetupAlreadyExists,
                "GPU Cloud Provider Setup already exists.",
                false,
            ),
            ProviderSetupError::InvalidProviderApiKey => Self::new(
                NativeCommandErrorCode::InvalidProviderApiKey,
                "Provider API Key is invalid.",
                false,
            ),
            ProviderSetupError::ProviderApiUnavailable => Self::new(
                NativeCommandErrorCode::ProviderApiUnavailable,
                "Provider API is unavailable.",
                true,
            ),
            ProviderSetupError::SecureKeyringUnavailable => Self::new(
                NativeCommandErrorCode::SecureKeyringUnavailable,
                "Secure keyring is unavailable.",
                true,
            ),
            ProviderSetupError::LocalStorageUnavailable => Self::new(
                NativeCommandErrorCode::LocalStorageUnavailable,
                "Local storage is unavailable.",
                true,
            ),
        }
    }
}
