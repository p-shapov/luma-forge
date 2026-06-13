use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{
        types::secrets::{ApiKeyIdentityResponse, SetupApiKeyRequest},
        CommandResult,
    },
    diagnostics::{command_request_metadata, CommandLogScope},
    secrets_storage::stores::ApiSecret,
};

#[tauri::command]
#[specta::specta]
pub async fn setup_runpod_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let command_log =
        CommandLogScope::new("setup_runpod_api_key", command_request_metadata(&request));
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let api_key = ApiSecret::new(request.api_key).map_err(|error| command_log.failed(error))?;
    let identity = state
        .runpod_secrets
        .write(api_key)
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let command_log = CommandLogScope::new("get_runpod_api_key_identity", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let identity = state
        .runpod_secrets
        .identity()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_runpod_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let command_log = CommandLogScope::new("delete_runpod_api_key", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    state
        .runpod_secrets
        .remove()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let command_log = CommandLogScope::new(
        "setup_hugging_face_api_key",
        command_request_metadata(&request),
    );
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let api_key = ApiSecret::new(request.api_key).map_err(|error| command_log.failed(error))?;
    let identity = state
        .hugging_face_secrets
        .write(api_key)
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let command_log = CommandLogScope::new("get_hugging_face_api_key_identity", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let identity = state
        .hugging_face_secrets
        .identity()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hugging_face_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let command_log = CommandLogScope::new("delete_hugging_face_api_key", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    state
        .hugging_face_secrets
        .remove()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(())
}
