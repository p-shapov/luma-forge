pub(super) mod contracts;

use crate::{app_state::NativeAppState, domain::provider_setup::ProviderApiKey, provider_setup};

use crate::commands::CommandResult;
use contracts::{
    DeleteGpuCloudProviderSetupRequest, DeleteGpuCloudProviderSetupResponse,
    GetGpuCloudProviderSetupRequest, GetGpuCloudProviderSetupResponse,
    SetupGpuCloudProviderRequest, SetupGpuCloudProviderResponse,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_gpu_cloud_provider_setup(
    request: GetGpuCloudProviderSetupRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetGpuCloudProviderSetupResponse> {
    let provider_id = request.gpu_cloud_provider_id;
    app_state
        .provider_setup_service()
        .get_setup(provider_id)
        .await
        .map(|setup| GetGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: setup,
        })
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<SetupGpuCloudProviderResponse> {
    let provider_id = request.gpu_cloud_provider_id;
    let api_key = ProviderApiKey::new(request.provider_api_key)
        .map_err(|_| provider_setup::ProviderSetupError::InvalidProviderApiKey)?;
    let _guard = app_state
        .provider_setup_coordinator()
        .lock(&provider_id)
        .await;

    app_state
        .provider_setup_service()
        .setup(provider_id, api_key)
        .await
        .map(|setup| SetupGpuCloudProviderResponse {
            gpu_cloud_provider_setup: setup,
        })
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<DeleteGpuCloudProviderSetupResponse> {
    let provider_id = request.gpu_cloud_provider_id;
    let _guard = app_state
        .provider_setup_coordinator()
        .lock(&provider_id)
        .await;

    app_state
        .provider_setup_service()
        .delete_setup(provider_id)
        .map(|()| DeleteGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: None,
        })
        .map_err(Into::into)
}
