use tauri::State;

use crate::{
    app::state::NativeAppState,
    tauri_api::{errors::command_error, types::native::NativeStartupStatusResponse, CommandResult},
};

#[tauri::command]
#[specta::specta]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    const COMMAND: &str = "get_native_startup_status";
    super::tracing::run_sync_command(COMMAND, |trace_id| {
        log::info!(command = COMMAND; "tauri command started");
        let result = {
            let response = match state.startup_error() {
                Some(error) => NativeStartupStatusResponse::Failed {
                    error: command_error(&trace_id, error.clone()),
                },
                None => NativeStartupStatusResponse::Ready,
            };
            Ok(response)
        };
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
}
