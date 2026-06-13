use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{types::native::NativeStartupStatusResponse, CommandResult},
};

#[tauri::command]
#[specta::specta]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    Ok(match state.startup_error() {
        Some(error) => NativeStartupStatusResponse::Failed {
            error: error.clone(),
        },
        None => NativeStartupStatusResponse::Ready,
    })
}
