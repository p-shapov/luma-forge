use tauri::State;

use crate::{
    app::state::NativeAppState,
    tauri_api::{types::native::NativeStartupStatusResponse, CommandResult},
};

#[tauri::command]
#[specta::specta]
pub fn get_native_startup_status(
    state: State<'_, NativeAppState>,
) -> CommandResult<NativeStartupStatusResponse> {
    let response = match state.startup_error() {
        Some(error) => NativeStartupStatusResponse::Failed {
            error: error.clone().into(),
        },
        None => NativeStartupStatusResponse::Ready,
    };
    Ok(response)
}
