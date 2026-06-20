use tauri::State;

use crate::{
    app::state::NativeAppState,
    tauri_api::{
        diagnostics::{empty_command_request_metadata, start_command_trace},
        types::native::NativeStartupStatusResponse,
        CommandResult,
    },
};

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_native_startup_status", request_metadata = tracing::field::debug(empty_command_request_metadata()), trace_id = tracing::field::Empty)
)]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    let _trace_id = start_command_trace();
    let response = match state.startup_error() {
        Some(error) => NativeStartupStatusResponse::Failed {
            error: error.clone().into(),
        },
        None => NativeStartupStatusResponse::Ready,
    };
    Ok(response)
}
