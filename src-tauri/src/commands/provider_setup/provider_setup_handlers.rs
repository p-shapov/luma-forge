use crate::{
    domain::provider_setup::ProviderApiKey,
    provider::ProviderClientRegistry,
    provider_setup::{self, ProviderSetupCoordinator, ProviderSetupService},
    secrets::KeyringSecretStore,
};

use super::provider_setup_command_contracts::{
    DeleteGpuCloudProviderSetupRequest, DeleteGpuCloudProviderSetupResponse,
    GetGpuCloudProviderSetupRequest, GetGpuCloudProviderSetupResponse,
    SetupGpuCloudProviderRequest, SetupGpuCloudProviderResponse,
};
use crate::commands::CommandResult;
use tauri::State;

fn provider_setup_service() -> ProviderSetupService<KeyringSecretStore, ProviderClientRegistry> {
    ProviderSetupService::new(KeyringSecretStore, ProviderClientRegistry::default())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_gpu_cloud_provider_setup(
    request: GetGpuCloudProviderSetupRequest,
) -> CommandResult<GetGpuCloudProviderSetupResponse> {
    let provider_id = request.gpu_cloud_provider_id.into();
    provider_setup_service()
        .get_setup(provider_id)
        .await
        .map(|setup| GetGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: setup.map(Into::into),
        })
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
    coordinator: State<'_, ProviderSetupCoordinator>,
) -> CommandResult<SetupGpuCloudProviderResponse> {
    let provider_id = request.gpu_cloud_provider_id.into();
    let api_key = ProviderApiKey::new(request.provider_api_key)
        .map_err(|_| provider_setup::ProviderSetupError::InvalidProviderApiKey)?;
    let _guard = coordinator.lock(&provider_id).await;

    provider_setup_service()
        .setup(provider_id, api_key)
        .await
        .map(|setup| SetupGpuCloudProviderResponse {
            gpu_cloud_provider_setup: setup.into(),
        })
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
    coordinator: State<'_, ProviderSetupCoordinator>,
) -> CommandResult<DeleteGpuCloudProviderSetupResponse> {
    let provider_id = request.gpu_cloud_provider_id.into();
    let _guard = coordinator.lock(&provider_id).await;

    provider_setup_service()
        .delete_setup(provider_id)
        .map(|()| DeleteGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: None,
        })
        .map_err(Into::into)
}
