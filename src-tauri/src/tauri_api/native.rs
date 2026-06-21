use tauri::State;

use crate::{
    app::state::NativeAppState,
    tauri_api::{
        errors::{
            command_error, NativeInitializationCommandError, NativeInitializationCommandErrorCode,
        },
        types::native::NativeStartupStatusResponse,
        CommandResult,
    },
};

#[tauri::command]
#[specta::specta]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    super::tracing::run_sync_command("get_native_startup_status", |trace_id| {
        let response = match state.startup_error() {
            Some(error) => NativeStartupStatusResponse::Failed {
                error: command_error(
                    &trace_id,
                    NativeInitializationCommandError::from(error.clone()),
                    |error| NativeInitializationCommandErrorCode::from(error.clone()),
                ),
            },
            None => NativeStartupStatusResponse::Ready,
        };
        Ok(response)
    })
}
