use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{
        types::secrets::{ApiKeyIdentityResponse, SetupApiKeyRequest},
        CommandResult,
    },
    secrets_storage::stores::ApiSecret,
};

#[tauri::command]
#[specta::specta]
pub async fn setup_runpod_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready()?;
    let identity = state
        .runpod_secrets
        .write(ApiSecret::new(request.api_key)?)
        .await?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready()?;
    let identity = state.runpod_secrets.identity().await?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_runpod_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let state = state.ready()?;
    state.runpod_secrets.remove().await?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready()?;
    let identity = state
        .hugging_face_secrets
        .write(ApiSecret::new(request.api_key)?)
        .await?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse> {
    let state = state.ready()?;
    let identity = state.hugging_face_secrets.identity().await?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hugging_face_api_key(state: State<'_, NativeAppState>) -> CommandResult<()> {
    let state = state.ready()?;
    state.hugging_face_secrets.remove().await?;

    Ok(())
}
