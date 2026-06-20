use serde::{Deserialize, Serialize};
use specta::Type;

use crate::app::errors::AppInitializationError;

pub type CommandResult<T, Code = NativeInitializationCommandErrorCode> =
    Result<T, CommandError<Code>>;
pub type NativeCommandError = CommandError<NativeInitializationCommandErrorCode>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct CommandError<Code> {
    pub message: String,
    pub code: Code,
    pub trace_id: String,
}

impl NativeCommandError {
    pub fn native_initialization(error: NativeInitializationCommandError) -> Self {
        NativeInitializationCommandErrorCode::from(error).into()
    }
}

impl From<AppInitializationError> for NativeCommandError {
    fn from(error: AppInitializationError) -> Self {
        Self::native_initialization(error.into())
    }
}

impl<Code> CommandError<Code> {
    pub(crate) fn new(code: Code, message: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code,
            trace_id: trace_id.into(),
        }
    }
}

impl<Code> From<Code> for CommandError<Code>
where
    Code: ToString,
{
    fn from(error: Code) -> Self {
        let message = error.to_string();

        Self {
            message,
            code: error,
            trace_id: crate::shared::new_trace_id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum NativeInitializationCommandErrorCode {
    #[error("app data directory is unavailable")]
    AppDataDirectoryUnavailable,
    #[error("app data directory could not be created")]
    AppDataDirectoryCreateFailed,
    #[error("workspace storage could not be initialized")]
    WorkspaceStorageInitializationFailed,
    #[error("provider services could not be initialized")]
    ProviderServicesInitializationFailed,
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
    #[error("provider services could not be initialized: {message}")]
    ProviderServicesInitializationFailed { message: String },
    #[error("workspace lifecycle state could not be restored: {message}")]
    LifecycleStateRestoreFailed { message: String },
}

impl From<NativeInitializationCommandError> for NativeInitializationCommandErrorCode {
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
            NativeInitializationCommandError::ProviderServicesInitializationFailed { .. } => {
                Self::ProviderServicesInitializationFailed
            }
            NativeInitializationCommandError::LifecycleStateRestoreFailed { .. } => {
                Self::LifecycleStateRestoreFailed
            }
        }
    }
}

impl From<AppInitializationError> for NativeInitializationCommandError {
    fn from(error: AppInitializationError) -> Self {
        match error {
            AppInitializationError::AppDataDirectoryUnavailable { message } => {
                Self::AppDataDirectoryUnavailable { message }
            }
            AppInitializationError::AppDataDirectoryCreateFailed { path, message } => {
                Self::AppDataDirectoryCreateFailed { path, message }
            }
            AppInitializationError::WorkspaceStorageInitializationFailed { path, message } => {
                Self::WorkspaceStorageInitializationFailed { path, message }
            }
            AppInitializationError::ProviderServicesInitializationFailed { message } => {
                Self::ProviderServicesInitializationFailed { message }
            }
            AppInitializationError::LifecycleStateRestoreFailed { message } => {
                Self::LifecycleStateRestoreFailed { message }
            }
        }
    }
}
