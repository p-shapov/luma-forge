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
    commands::{
        types::secrets::{ApiKeyIdentityResponse, SetupApiKeyRequest},
        CommandResult,
    },
    diagnostics::{
        command_error, command_request_metadata, empty_command_request_metadata,
        native_command_error,
    },
    secrets::stores::ApiSecret,
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
) -> CommandResult<ApiKeyIdentityResponse, SetupRunpodApiKeyErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "setup_runpod_api_key",
            error,
            SetupRunpodApiKeyErrorCode::NativeInitializationFailed,
        )
    })?;
    let api_key = ApiSecret::new(request.api_key).map_err(|error| {
        command_error("setup_runpod_api_key", error, setup_runpod_api_key_error)
    })?;
    let identity = state.runpod_secrets.write(api_key).await.map_err(|error| {
        command_error("setup_runpod_api_key", error, setup_runpod_api_key_error)
    })?;

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
) -> CommandResult<ApiKeyIdentityResponse, GetRunpodApiKeyIdentityErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_runpod_api_key_identity",
            error,
            GetRunpodApiKeyIdentityErrorCode::NativeInitializationFailed,
        )
    })?;
    let identity = state.runpod_secrets.identity().await.map_err(|error| {
        command_error(
            "get_runpod_api_key_identity",
            error,
            get_runpod_api_key_identity_error,
        )
    })?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "delete_runpod_api_key", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn delete_runpod_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteRunpodApiKeyErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "delete_runpod_api_key",
            error,
            DeleteRunpodApiKeyErrorCode::NativeInitializationFailed,
        )
    })?;
    state.runpod_secrets.remove().await.map_err(|error| {
        command_error("delete_runpod_api_key", error, delete_runpod_api_key_error)
    })?;

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
) -> CommandResult<ApiKeyIdentityResponse, SetupHuggingFaceApiKeyErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "setup_hugging_face_api_key",
            error,
            SetupHuggingFaceApiKeyErrorCode::NativeInitializationFailed,
        )
    })?;
    let api_key = ApiSecret::new(request.api_key).map_err(|error| {
        command_error(
            "setup_hugging_face_api_key",
            error,
            setup_hugging_face_api_key_error,
        )
    })?;
    let identity = state
        .hugging_face_secrets
        .write(api_key)
        .await
        .map_err(|error| {
            command_error(
                "setup_hugging_face_api_key",
                error,
                setup_hugging_face_api_key_error,
            )
        })?;

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
) -> CommandResult<ApiKeyIdentityResponse, GetHuggingFaceApiKeyIdentityErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_hugging_face_api_key_identity",
            error,
            GetHuggingFaceApiKeyIdentityErrorCode::NativeInitializationFailed,
        )
    })?;
    let identity = state
        .hugging_face_secrets
        .identity()
        .await
        .map_err(|error| {
            command_error(
                "get_hugging_face_api_key_identity",
                error,
                get_hugging_face_api_key_identity_error,
            )
        })?;

    Ok(identity.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "delete_hugging_face_api_key", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn delete_hugging_face_api_key(
    state: State<'_, NativeAppState>,
) -> CommandResult<(), DeleteHuggingFaceApiKeyErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "delete_hugging_face_api_key",
            error,
            DeleteHuggingFaceApiKeyErrorCode::NativeInitializationFailed,
        )
    })?;
    state.hugging_face_secrets.remove().await.map_err(|error| {
        command_error(
            "delete_hugging_face_api_key",
            error,
            delete_hugging_face_api_key_error,
        )
    })?;

    Ok(())
}
