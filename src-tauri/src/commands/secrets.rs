use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{
        types::secrets::{ApiKeyIdentityResponse, SetupApiKeyRequest},
        CommandResult,
    },
    diagnostics::{
        command_error, command_request_metadata, empty_command_request_metadata,
        native_command_error,
    },
    secrets_storage::stores::ApiSecret,
};

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "setup_runpod_api_key", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn setup_runpod_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let api_key = ApiSecret::new(request.api_key)
        .map_err(|error| command_error("setup_runpod_api_key", error))?;
    let identity = state
        .runpod_secrets
        .write(api_key)
        .await
        .map_err(|error| command_error("setup_runpod_api_key", error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_runpod_api_key_identity", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let identity = state
        .runpod_secrets
        .identity()
        .await
        .map_err(|error| command_error("get_runpod_api_key_identity", error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "delete_runpod_api_key", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn delete_runpod_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let state = state.ready().map_err(native_command_error)?;
    state
        .runpod_secrets
        .remove()
        .await
        .map_err(|error| command_error("delete_runpod_api_key", error))?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "setup_hugging_face_api_key", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let api_key = ApiSecret::new(request.api_key)
        .map_err(|error| command_error("setup_hugging_face_api_key", error))?;
    let identity = state
        .hugging_face_secrets
        .write(api_key)
        .await
        .map_err(|error| command_error("setup_hugging_face_api_key", error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_hugging_face_api_key_identity", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let identity = state
        .hugging_face_secrets
        .identity()
        .await
        .map_err(|error| command_error("get_hugging_face_api_key_identity", error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "delete_hugging_face_api_key", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn delete_hugging_face_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let state = state.ready().map_err(native_command_error)?;
    state
        .hugging_face_secrets
        .remove()
        .await
        .map_err(|error| command_error("delete_hugging_face_api_key", error))?;

    Ok(())
}
