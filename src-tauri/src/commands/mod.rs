//! Tauri command boundary exposed to the React client.

use crate::{
    provider::ProviderRegistry,
    provider_setup::{
        DeleteGpuCloudProviderSetupRequest, DeleteGpuCloudProviderSetupResponse,
        GetGpuCloudProviderSetupRequest, GetGpuCloudProviderSetupResponse, NativeCommandError,
        ProviderSetupService, SetupGpuCloudProviderRequest, SetupGpuCloudProviderResponse,
    },
    secrets::KeyringSecretStore,
};
use tauri_specta::{collect_commands, Builder};

#[cfg(any(debug_assertions, test))]
mod bindings;

#[cfg(any(debug_assertions, test))]
pub(crate) use bindings::export_typescript_bindings;

type CommandResult<T> = Result<T, NativeCommandError>;

fn provider_setup_service() -> ProviderSetupService<KeyringSecretStore, ProviderRegistry> {
    ProviderSetupService::new(KeyringSecretStore, ProviderRegistry::default())
}

#[tauri::command]
#[specta::specta]
async fn get_gpu_cloud_provider_setup(
    request: GetGpuCloudProviderSetupRequest,
) -> CommandResult<GetGpuCloudProviderSetupResponse> {
    provider_setup_service()
        .get_setup(request)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
async fn setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
) -> CommandResult<SetupGpuCloudProviderResponse> {
    provider_setup_service()
        .setup(request)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
fn delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
) -> CommandResult<DeleteGpuCloudProviderSetupResponse> {
    provider_setup_service()
        .delete_setup(request)
        .map_err(Into::into)
}

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        get_gpu_cloud_provider_setup,
        setup_gpu_cloud_provider,
        delete_gpu_cloud_provider_setup
    ])
}
