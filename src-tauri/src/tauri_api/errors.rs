use serde::{Deserialize, Serialize};
use specta::Type;

use crate::diagnostics;

pub type CommandResult<T, Code = NativeInitializationCommandErrorCode> =
    Result<T, CommandError<Code>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("command failed")]
#[serde(rename_all = "camelCase")]
pub struct CommandError<Code> {
    pub code: Code,
    pub trace_id: String,
}

impl<Code> CommandError<Code> {
    pub(crate) fn new(code: Code, trace_id: impl Into<String>) -> Self {
        Self {
            code,
            trace_id: trace_id.into(),
        }
    }
}

pub(crate) trait CommandErrorCode: Sized {
    fn from_diagnostics_code(code: &str) -> Self;
}

pub(crate) fn is_native_initialization_diagnostics_code(code: &str) -> bool {
    matches!(
        code,
        "app_data_directory_unavailable"
            | "app_data_directory_create_failed"
            | "diagnostics_initialization_failed"
            | "workspace_storage_initialization_failed"
            | "provider_services_initialization_failed"
            | "lifecycle_state_restore_failed"
    )
}

pub(crate) fn command_error<E, Code>(trace_id: &str, error: E) -> CommandError<Code>
where
    E: diagnostics::HasDiagnosticCode + 'static,
    Code: CommandErrorCode,
{
    let diagnostics = diagnostics::error_diagnostics(&error);
    let code = Code::from_diagnostics_code(&diagnostics.code);

    CommandError::new(code, trace_id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeInitializationCommandErrorCode {
    NativeInitializationFailed,
    CommandError,
}

impl CommandErrorCode for NativeInitializationCommandErrorCode {
    fn from_diagnostics_code(code: &str) -> Self {
        match code {
            code if is_native_initialization_diagnostics_code(code) => {
                Self::NativeInitializationFailed
            }
            _ => Self::CommandError,
        }
    }
}
