mod errors;

use tauri::State;

use errors::{
    delete_hugging_face_api_key_error, delete_runpod_api_key_error,
    get_hugging_face_api_key_identity_error, get_runpod_api_key_identity_error,
    setup_hugging_face_api_key_error, setup_runpod_api_key_error, DeleteHuggingFaceApiKeyErrorCode,
    DeleteRunpodApiKeyErrorCode, GetHuggingFaceApiKeyIdentityErrorCode,
    GetRunpodApiKeyIdentityErrorCode, SetupHuggingFaceApiKeyErrorCode, SetupRunpodApiKeyErrorCode,
};

use crate::{
    app::state::NativeAppState,
    secrets::stores::ApiSecret,
    tauri_api::{
        errors::{command_error, NativeCommandError, TraceId},
        types::secrets::{ApiKeyIdentityResponse, SetupApiKeyRequest},
        CommandResult,
    },
};

#[tauri::command]
#[specta::specta]
pub async fn setup_runpod_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse, SetupRunpodApiKeyErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            SetupRunpodApiKeyErrorCode::NativeInitializationFailed
        })
    })?;
    let api_key = ApiSecret::new(request.api_key)
        .map_err(|error| command_error(&trace_id, error, setup_runpod_api_key_error))?;
    let identity = state
        .runpod_secrets
        .write(api_key)
        .await
        .map_err(|error| command_error(&trace_id, error, setup_runpod_api_key_error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetRunpodApiKeyIdentityErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            GetRunpodApiKeyIdentityErrorCode::NativeInitializationFailed
        })
    })?;
    let identity = state
        .runpod_secrets
        .identity()
        .await
        .map_err(|error| command_error(&trace_id, error, get_runpod_api_key_identity_error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_runpod_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteRunpodApiKeyErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            DeleteRunpodApiKeyErrorCode::NativeInitializationFailed
        })
    })?;
    state
        .runpod_secrets
        .remove()
        .await
        .map_err(|error| command_error(&trace_id, error, delete_runpod_api_key_error))?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse, SetupHuggingFaceApiKeyErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            SetupHuggingFaceApiKeyErrorCode::NativeInitializationFailed
        })
    })?;
    let api_key = ApiSecret::new(request.api_key)
        .map_err(|error| command_error(&trace_id, error, setup_hugging_face_api_key_error))?;
    let identity = state
        .hugging_face_secrets
        .write(api_key)
        .await
        .map_err(|error| command_error(&trace_id, error, setup_hugging_face_api_key_error))?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetHuggingFaceApiKeyIdentityErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            GetHuggingFaceApiKeyIdentityErrorCode::NativeInitializationFailed
        })
    })?;
    let identity = state
        .hugging_face_secrets
        .identity()
        .await
        .map_err(|error| {
            command_error(&trace_id, error, get_hugging_face_api_key_identity_error)
        })?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hugging_face_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteHuggingFaceApiKeyErrorCode> {
    let trace_id = TraceId::random().to_string();
    let state = state.ready().map_err(|error| {
        command_error(&trace_id, NativeCommandError::from(error), |_| {
            DeleteHuggingFaceApiKeyErrorCode::NativeInitializationFailed
        })
    })?;
    state
        .hugging_face_secrets
        .remove()
        .await
        .map_err(|error| command_error(&trace_id, error, delete_hugging_face_api_key_error))?;

    Ok(())
}
