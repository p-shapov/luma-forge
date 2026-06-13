use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{types::native::NativeStartupStatusResponse, CommandResult},
    diagnostics::CommandLogScope,
};

#[tauri::command]
#[specta::specta]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    let command_log = CommandLogScope::new("get_native_startup_status", Vec::new());
    let response = match state.startup_error() {
        Some(error) => NativeStartupStatusResponse::Failed {
            error: error.clone(),
        },
        None => NativeStartupStatusResponse::Ready,
    };
    command_log.completed();
    Ok(response)
}
