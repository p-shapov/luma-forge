mod errors;

use tauri::State;

use errors::{
    DeleteHuggingFaceApiKeyErrorCode, DeleteRunpodApiKeyErrorCode,
    GetHuggingFaceApiKeyIdentityErrorCode, GetRunpodApiKeyIdentityErrorCode,
    SetupHuggingFaceApiKeyErrorCode, SetupRunpodApiKeyErrorCode,
};

use crate::{
    app::state::NativeAppState,
    secrets::stores::ApiSecret,
    tauri_api::{
        errors::command_error,
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
    const COMMAND: &str = "setup_runpod_api_key";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let api_key =
                ApiSecret::new(request.api_key).map_err(|error| command_error(&trace_id, error))?;
            let identity = state.runpod_secrets.write(api_key).await.map_err(|error| {
                log::error!(
                    command = COMMAND,
                    provider = "runpod",
                    error = crate::diagnostics::error_diagnostics_log_json(&error);
                    "api key setup service failed"
                );
                command_error(&trace_id, error)
            })?;

            Ok(identity.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetRunpodApiKeyIdentityErrorCode> {
    const COMMAND: &str = "get_runpod_api_key_identity";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let identity = state.runpod_secrets.identity().await.map_err(|error| {
                log::error!(
                    command = COMMAND,
                    provider = "runpod",
                    error = crate::diagnostics::error_diagnostics_log_json(&error);
                    "api key identity service failed"
                );
                command_error(&trace_id, error)
            })?;

            Ok(identity.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_runpod_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteRunpodApiKeyErrorCode> {
    const COMMAND: &str = "delete_runpod_api_key";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            state.runpod_secrets.remove().await.map_err(|error| {
                log::error!(
                    command = COMMAND,
                    provider = "runpod",
                    error = crate::diagnostics::error_diagnostics_log_json(&error);
                    "api key deletion service failed"
                );
                command_error(&trace_id, error)
            })?;

            Ok(())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn setup_hugging_face_api_key(
    state: State<'_, NativeAppState>,
    request: SetupApiKeyRequest,
) -> CommandResult<ApiKeyIdentityResponse, SetupHuggingFaceApiKeyErrorCode> {
    const COMMAND: &str = "setup_hugging_face_api_key";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let api_key =
                ApiSecret::new(request.api_key).map_err(|error| command_error(&trace_id, error))?;
            let identity = state
                .hugging_face_secrets
                .write(api_key)
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        provider = "hugging_face",
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "api key setup service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(identity.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_hugging_face_api_key_identity(
    state: State<'_, NativeAppState>,
) -> CommandResult<ApiKeyIdentityResponse, GetHuggingFaceApiKeyIdentityErrorCode> {
    const COMMAND: &str = "get_hugging_face_api_key_identity";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let identity = state
                .hugging_face_secrets
                .identity()
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        provider = "hugging_face",
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "api key identity service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(identity.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hugging_face_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteHuggingFaceApiKeyErrorCode> {
    const COMMAND: &str = "delete_hugging_face_api_key";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            state.hugging_face_secrets.remove().await.map_err(|error| {
                log::error!(
                    command = COMMAND,
                    provider = "hugging_face",
                    error = crate::diagnostics::error_diagnostics_log_json(&error);
                    "api key deletion service failed"
                );
                command_error(&trace_id, error)
            })?;

            Ok(())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}
