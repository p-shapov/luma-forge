use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{types::native::NativeStartupStatusResponse, CommandResult},
    diagnostics::empty_command_request_metadata,
};

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_native_startup_status", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    let response = match state.startup_error() {
        Some(error) => NativeStartupStatusResponse::Failed {
            error: error.clone(),
        },
        None => NativeStartupStatusResponse::Ready,
    };
    Ok(response)
}
