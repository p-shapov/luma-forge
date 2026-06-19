use serde::{Deserialize, Serialize};
use specta::Type;

pub type CommandResult<T, Code = NativeInitializationCommandErrorCode> =
    Result<T, CommandError<Code>>;
pub type NativeCommandError = CommandError<NativeInitializationCommandErrorCode>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct CommandError<Code> {
    pub message: String,
    pub code: Code,
    pub diagnostic_id: String,
}

impl NativeCommandError {
    pub fn native_initialization(error: NativeInitializationCommandError) -> Self {
        NativeInitializationCommandErrorCode::from(error).into()
    }
}

impl<Code> CommandError<Code> {
    pub(crate) fn new(
        code: Code,
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

impl<Code> From<Code> for CommandError<Code>
where
    Code: ToString,
{
    fn from(error: Code) -> Self {
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
pub enum NativeInitializationCommandErrorCode {
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
            NativeInitializationCommandError::LifecycleStateRestoreFailed { .. } => {
                Self::LifecycleStateRestoreFailed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_error_serializes_message_and_tagged_error() {
        let error = NativeCommandError::new(
            NativeInitializationCommandErrorCode::LifecycleStateRestoreFailed,
            "workspace lifecycle state could not be restored",
            "diag-123",
        );

        let json = serde_json::to_string(&error).expect("command error json");

        assert_eq!(
            json,
            r#"{"message":"workspace lifecycle state could not be restored","code":"lifecycle_state_restore_failed","diagnosticId":"diag-123"}"#
        );
    }
}
