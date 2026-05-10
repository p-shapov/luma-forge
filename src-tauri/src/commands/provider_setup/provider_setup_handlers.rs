use crate::{
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
    provider_setup_service()
        .get_setup(request.into())
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
    coordinator: State<'_, ProviderSetupCoordinator>,
) -> CommandResult<SetupGpuCloudProviderResponse> {
    let request: provider_setup::SetupGpuCloudProviderRequest = request.into();
    let provider_id = request.gpu_cloud_provider_id.into();
    let _guard = coordinator.lock(&provider_id).await;

    provider_setup_service()
        .setup(request)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
    coordinator: State<'_, ProviderSetupCoordinator>,
) -> CommandResult<DeleteGpuCloudProviderSetupResponse> {
    let request: provider_setup::DeleteGpuCloudProviderSetupRequest = request.into();
    let provider_id = request.gpu_cloud_provider_id.into();
    let _guard = coordinator.lock(&provider_id).await;

    provider_setup_service()
        .delete_setup(request)
        .map(Into::into)
        .map_err(Into::into)
}
