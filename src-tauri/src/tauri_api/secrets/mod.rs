mod errors;

use tauri::State;

use errors::{
    DeleteHuggingFaceApiKeyCommandError, DeleteHuggingFaceApiKeyErrorCode,
    DeleteRunpodApiKeyCommandError, DeleteRunpodApiKeyErrorCode,
    GetHuggingFaceApiKeyIdentityCommandError, GetHuggingFaceApiKeyIdentityErrorCode,
    GetRunpodApiKeyIdentityCommandError, GetRunpodApiKeyIdentityErrorCode,
    SetupHuggingFaceApiKeyCommandError, SetupHuggingFaceApiKeyErrorCode,
    SetupRunpodApiKeyCommandError, SetupRunpodApiKeyErrorCode,
};

use crate::{
    app::state::NativeAppState,
    secrets::stores::ApiSecret,
    tauri_api::{
        errors::{command_error, NativeInitializationCommandError},
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
    super::tracing::run_async_command("setup_runpod_api_key", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                SetupRunpodApiKeyCommandError::from(NativeInitializationCommandError::from(error)),
            )
        })?;
        let api_key = ApiSecret::new(request.api_key).map_err(|error| {
            command_error(&trace_id, SetupRunpodApiKeyCommandError::from(error))
        })?;
        let identity = state.runpod_secrets.write(api_key).await.map_err(|error| {
            command_error(&trace_id, SetupRunpodApiKeyCommandError::from(error))
        })?;

        Ok(identity.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetRunpodApiKeyIdentityErrorCode> {
    super::tracing::run_async_command("get_runpod_api_key_identity", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                GetRunpodApiKeyIdentityCommandError::from(NativeInitializationCommandError::from(
                    error,
                )),
            )
        })?;
        let identity = state.runpod_secrets.identity().await.map_err(|error| {
            command_error(&trace_id, GetRunpodApiKeyIdentityCommandError::from(error))
        })?;

        Ok(identity.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_runpod_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteRunpodApiKeyErrorCode> {
    super::tracing::run_async_command("delete_runpod_api_key", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                DeleteRunpodApiKeyCommandError::from(NativeInitializationCommandError::from(error)),
            )
        })?;
        state.runpod_secrets.remove().await.map_err(|error| {
            command_error(&trace_id, DeleteRunpodApiKeyCommandError::from(error))
        })?;

        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse, SetupHuggingFaceApiKeyErrorCode> {
    super::tracing::run_async_command("setup_hugging_face_api_key", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                SetupHuggingFaceApiKeyCommandError::from(NativeInitializationCommandError::from(
                    error,
                )),
            )
        })?;
        let api_key = ApiSecret::new(request.api_key).map_err(|error| {
            command_error(&trace_id, SetupHuggingFaceApiKeyCommandError::from(error))
        })?;
        let identity = state
            .hugging_face_secrets
            .write(api_key)
            .await
            .map_err(|error| {
                command_error(&trace_id, SetupHuggingFaceApiKeyCommandError::from(error))
            })?;

        Ok(identity.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetHuggingFaceApiKeyIdentityErrorCode> {
    super::tracing::run_async_command("get_hugging_face_api_key_identity", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                GetHuggingFaceApiKeyIdentityCommandError::from(
                    NativeInitializationCommandError::from(error),
                ),
            )
        })?;
        let identity = state
            .hugging_face_secrets
            .identity()
            .await
            .map_err(|error| {
                command_error(
                    &trace_id,
                    GetHuggingFaceApiKeyIdentityCommandError::from(error),
                )
            })?;

        Ok(identity.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hugging_face_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteHuggingFaceApiKeyErrorCode> {
    super::tracing::run_async_command("delete_hugging_face_api_key", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                DeleteHuggingFaceApiKeyCommandError::from(NativeInitializationCommandError::from(
                    error,
                )),
            )
        })?;
        state.hugging_face_secrets.remove().await.map_err(|error| {
            command_error(&trace_id, DeleteHuggingFaceApiKeyCommandError::from(error))
        })?;

        Ok(())
    })
    .await
}
