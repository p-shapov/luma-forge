pub(super) mod contracts;

use crate::{
    app_state::NativeAppState, commands::logging::CommandLog,
    domain::hugging_face_setup::HuggingFaceApiKey, hugging_face_setup,
};

use crate::commands::{error::NativeCommandError, CommandResult};
use contracts::{
    DeleteHuggingFaceApiKeySetupRequest, DeleteHuggingFaceApiKeySetupResponse,
    GetHuggingFaceApiKeySetupRequest, GetHuggingFaceApiKeySetupResponse,
    SetupHuggingFaceApiKeyRequest, SetupHuggingFaceApiKeyResponse,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_hugging_face_api_key_setup(
    _request: GetHuggingFaceApiKeySetupRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetHuggingFaceApiKeySetupResponse> {
    let command_log = CommandLog::new("get_hugging_face_api_key_setup").start();
    let result = app_state
        .hugging_face_setup_service()
        .map_err(NativeCommandError::from)?
        .get_setup()
        .await
        .map(|setup| GetHuggingFaceApiKeySetupResponse {
            hugging_face_api_key_setup: setup,
        })
        .map_err(Into::into);
    command_log.finish(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_hugging_face_api_key(
    request: SetupHuggingFaceApiKeyRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<SetupHuggingFaceApiKeyResponse> {
    let command_log = CommandLog::new("setup_hugging_face_api_key").start();
    let result = async {
        let api_key = HuggingFaceApiKey::new(request.hugging_face_api_key)
            .map_err(|_| hugging_face_setup::HuggingFaceSetupError::HuggingFaceApiKeyRequired)?;

        app_state
            .hugging_face_setup_service()
            .map_err(NativeCommandError::from)?
            .setup(api_key)
            .await
            .map(|setup| SetupHuggingFaceApiKeyResponse {
                hugging_face_api_key_setup: setup,
            })
            .map_err(Into::into)
    }
    .await;
    command_log.finish(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_hugging_face_api_key_setup(
    _request: DeleteHuggingFaceApiKeySetupRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<DeleteHuggingFaceApiKeySetupResponse> {
    let command_log = CommandLog::new("delete_hugging_face_api_key_setup").start();
    let result = app_state
        .hugging_face_setup_service()
        .map_err(NativeCommandError::from)?
        .delete_setup()
        .await
        .map(|()| DeleteHuggingFaceApiKeySetupResponse {
            hugging_face_api_key_setup: None,
        })
        .map_err(Into::into);
    command_log.finish(result)
}
