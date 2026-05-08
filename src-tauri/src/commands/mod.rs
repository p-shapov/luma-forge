//! Tauri command boundary exposed to the React client.

use crate::{
    bundled::bundled_catalog::BundledCatalogReader,
    provider::ProviderRegistry,
    provider_setup::{
        DeleteGpuCloudProviderSetupRequest, DeleteGpuCloudProviderSetupResponse,
        GetGpuCloudProviderSetupRequest, GetGpuCloudProviderSetupResponse, NativeCommandError,
        ProviderSetupService, SetupGpuCloudProviderRequest, SetupGpuCloudProviderResponse,
    },
    secrets::KeyringSecretStore,
    workspace::{
        workspace_catalog::{SqliteWorkspaceCatalog, UnavailableWorkspaceCatalog},
        workspace_setup::{
            CreateWorkspaceRequest, CreateWorkspaceResponse, GetEndpointProfilesResponse,
            GetProviderInventoryRequest, GetProviderInventoryResponse,
            GetProvisioningProfilesResponse, GetWorkflowCatalogResponse,
            GetWorkspaceCatalogResponse, WorkspaceSetupError, WorkspaceSetupService,
        },
    },
};
use tauri::{AppHandle, Manager};
use tauri_specta::{collect_commands, Builder};

#[cfg(any(debug_assertions, test))]
mod bindings;

#[cfg(any(debug_assertions, test))]
pub(crate) use bindings::export_typescript_bindings;

type CommandResult<T> = Result<T, NativeCommandError>;

fn provider_setup_service() -> ProviderSetupService<KeyringSecretStore, ProviderRegistry> {
    ProviderSetupService::new(KeyringSecretStore, ProviderRegistry::default())
}

fn workspace_setup_read_service() -> WorkspaceSetupService<
    BundledCatalogReader,
    KeyringSecretStore,
    ProviderRegistry,
    UnavailableWorkspaceCatalog,
> {
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderRegistry::default(),
        UnavailableWorkspaceCatalog,
    )
}

async fn sqlite_workspace_catalog(
    app: &AppHandle,
) -> Result<SqliteWorkspaceCatalog, NativeCommandError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| WorkspaceSetupError::LocalStorageUnavailable)?;
    SqliteWorkspaceCatalog::connect(data_dir.join("workspace-catalog.sqlite"))
        .await
        .map_err(Into::into)
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

#[tauri::command]
#[specta::specta]
fn get_workflow_catalog() -> CommandResult<GetWorkflowCatalogResponse> {
    workspace_setup_read_service()
        .get_workflow_catalog()
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
fn get_provisioning_profiles() -> CommandResult<GetProvisioningProfilesResponse> {
    workspace_setup_read_service()
        .get_provisioning_profiles()
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
fn get_endpoint_profiles() -> CommandResult<GetEndpointProfilesResponse> {
    workspace_setup_read_service()
        .get_endpoint_profiles()
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
async fn get_provider_inventory(
    request: GetProviderInventoryRequest,
) -> CommandResult<GetProviderInventoryResponse> {
    workspace_setup_read_service()
        .get_provider_inventory(request)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
async fn get_workspace_catalog(app: AppHandle) -> CommandResult<GetWorkspaceCatalogResponse> {
    let workspace_catalog = sqlite_workspace_catalog(&app).await?;
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderRegistry::default(),
        workspace_catalog,
    )
    .get_workspace_catalog()
    .await
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
async fn create_workspace(
    app: AppHandle,
    request: CreateWorkspaceRequest,
) -> CommandResult<CreateWorkspaceResponse> {
    let workspace_catalog = sqlite_workspace_catalog(&app).await?;
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderRegistry::default(),
        workspace_catalog,
    )
    .create_workspace(request)
    .await
    .map_err(Into::into)
}

pub(crate) fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        get_gpu_cloud_provider_setup,
        setup_gpu_cloud_provider,
        delete_gpu_cloud_provider_setup,
        get_workflow_catalog,
        get_provisioning_profiles,
        get_endpoint_profiles,
        get_provider_inventory,
        get_workspace_catalog,
        create_workspace
    ])
}
