use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use crate::{
    app_state::AppState,
    domain::{
        native_error::NativeCommandError,
        provider_setup::{GpuCloudProviderId, GpuCloudProviderSetup},
    },
};

#[derive(Debug, Deserialize, Type)]
pub(crate) struct GetGpuCloudProviderSetupRequest {
    gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Serialize, Type)]
pub(crate) struct GetGpuCloudProviderSetupResponse {
    gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Debug, Serialize, Type)]
pub(crate) struct DeleteGpuCloudProviderSetupResponse {
    gpu_cloud_provider_setup: Option<GpuCloudProviderSetup>,
}

#[derive(Debug, Deserialize, Type)]
pub(crate) struct DeleteGpuCloudProviderSetupRequest {
    gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Deserialize, Type)]
pub(crate) struct SetupGpuCloudProviderRequest {
    gpu_cloud_provider_id: GpuCloudProviderId,
    provider_api_key: String,
}

#[derive(Debug, Serialize, Type)]
pub(crate) struct SetupGpuCloudProviderResponse {
    gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

#[derive(Debug, Serialize, Type)]
pub(crate) struct SyncGpuCloudProviderSetupResponse {
    gpu_cloud_provider_setup: GpuCloudProviderSetup,
}

#[derive(Debug, Deserialize, Type)]
pub(crate) struct SyncGpuCloudProviderSetupRequest {
    gpu_cloud_provider_id: GpuCloudProviderId,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_gpu_cloud_provider_setup(
    request: GetGpuCloudProviderSetupRequest,
    state: State<'_, AppState>,
) -> Result<GetGpuCloudProviderSetupResponse, NativeCommandError> {
    let gpu_cloud_provider_setup = state
        .provider_setup_service()
        .get_setup(request.gpu_cloud_provider_id)
        .await
        .map_err(NativeCommandError::from)?;

    Ok(GetGpuCloudProviderSetupResponse {
        gpu_cloud_provider_setup,
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
    state: State<'_, AppState>,
) -> Result<SetupGpuCloudProviderResponse, NativeCommandError> {
    let gpu_cloud_provider_setup = state
        .provider_setup_service()
        .setup_provider(request.gpu_cloud_provider_id, request.provider_api_key)
        .await
        .map_err(NativeCommandError::from)?;

    Ok(SetupGpuCloudProviderResponse {
        gpu_cloud_provider_setup,
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_gpu_cloud_provider_setup(
    request: SyncGpuCloudProviderSetupRequest,
    state: State<'_, AppState>,
) -> Result<SyncGpuCloudProviderSetupResponse, NativeCommandError> {
    let gpu_cloud_provider_setup = state
        .provider_setup_service()
        .sync_setup(request.gpu_cloud_provider_id)
        .await
        .map_err(NativeCommandError::from)?;

    Ok(SyncGpuCloudProviderSetupResponse {
        gpu_cloud_provider_setup,
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
    state: State<'_, AppState>,
) -> Result<DeleteGpuCloudProviderSetupResponse, NativeCommandError> {
    let gpu_cloud_provider_setup = state
        .provider_setup_service()
        .delete_setup(request.gpu_cloud_provider_id)
        .await
        .map_err(NativeCommandError::from)?;

    Ok(DeleteGpuCloudProviderSetupResponse {
        gpu_cloud_provider_setup,
    })
}
