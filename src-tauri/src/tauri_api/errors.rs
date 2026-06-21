use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{app::errors::AppInitializationError, diagnostics};

pub type CommandResult<T, Code = NativeInitializationCommandErrorCode> =
    Result<T, CommandError<Code>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct CommandError<Code> {
    pub message: String,
    pub code: Code,
    pub trace_id: String,
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

pub(crate) trait CommandErrorCode: Sized {
    fn from_diagnostics_code(code: &str) -> Self;
    fn as_str(&self) -> &'static str;
}

pub(crate) fn command_error<E, Code>(trace_id: &str, error: E) -> CommandError<Code>
where
    E: std::error::Error + Serialize + 'static,
    Code: CommandErrorCode,
{
    let diagnostics = diagnostics::error_diagnostics(&error, "command_error");
    let code = Code::from_diagnostics_code(&diagnostics.code);

    CommandError::new(code, diagnostics.message, trace_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum NativeInitializationCommandErrorCode {
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
    #[error("command error")]
    CommandError,
}

impl CommandErrorCode for NativeInitializationCommandErrorCode {
    fn from_diagnostics_code(code: &str) -> Self {
        match code {
            "app_data_directory_unavailable" => Self::AppDataDirectoryUnavailable,
            "app_data_directory_create_failed" => Self::AppDataDirectoryCreateFailed,
            "diagnostics_initialization_failed" => Self::DiagnosticsInitializationFailed,
            "workspace_storage_initialization_failed" => Self::WorkspaceStorageInitializationFailed,
            "provider_services_initialization_failed" => Self::ProviderServicesInitializationFailed,
            "lifecycle_state_restore_failed" => Self::LifecycleStateRestoreFailed,
            _ => Self::CommandError,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::AppDataDirectoryUnavailable => "app_data_directory_unavailable",
            Self::AppDataDirectoryCreateFailed => "app_data_directory_create_failed",
            Self::DiagnosticsInitializationFailed => "diagnostics_initialization_failed",
            Self::WorkspaceStorageInitializationFailed => "workspace_storage_initialization_failed",
            Self::ProviderServicesInitializationFailed => "provider_services_initialization_failed",
            Self::LifecycleStateRestoreFailed => "lifecycle_state_restore_failed",
            Self::CommandError => "command_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum NativeInitializationCommandError {
    #[error("app data directory is unavailable: {message}")]
    AppDataDirectoryUnavailable { message: String },
    #[error("app data directory could not be created at {path}: {message}")]
    AppDataDirectoryCreateFailed { path: String, message: String },
    #[error("native diagnostics could not be initialized: {message}")]
    DiagnosticsInitializationFailed { message: String },
    #[error("workspace storage could not be initialized at {path}: {message}")]
    WorkspaceStorageInitializationFailed { path: String, message: String },
    #[error("provider services could not be initialized: {message}")]
    ProviderServicesInitializationFailed { message: String },
    #[error("workspace lifecycle state could not be restored: {message}")]
    LifecycleStateRestoreFailed { message: String },
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
            AppInitializationError::DiagnosticsInitializationFailed { message } => {
                Self::DiagnosticsInitializationFailed { message }
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
