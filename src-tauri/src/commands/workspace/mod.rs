pub(super) mod contracts;

use crate::{
    bundled_catalog::reader::BundledCatalogReader,
    commands::error::NativeCommandError,
    provider::ProviderClientRegistry,
    provider_setup::ProviderSetupCoordinator,
    secrets::KeyringSecretStore,
    workspace_catalog::{repository::UnavailableWorkspaceCatalog, sqlite::SqliteWorkspaceCatalog},
    workspace_setup::{
        contracts::CreateWorkspaceInput, error::WorkspaceSetupError, WorkspaceSetupService,
    },
};
use tauri::{AppHandle, Manager, State};

use crate::commands::CommandResult;
use contracts::{
    CreateWorkspaceRequest, CreateWorkspaceResponse, GetEndpointProfilesResponse,
    GetProviderInventoryRequest, GetProviderInventoryResponse, GetProvisioningProfilesResponse,
    GetWorkflowCatalogResponse, GetWorkspaceCatalogResponse,
};

fn workspace_setup_read_service() -> WorkspaceSetupService<
    BundledCatalogReader,
    KeyringSecretStore,
    ProviderClientRegistry,
    UnavailableWorkspaceCatalog,
> {
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderClientRegistry::default(),
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
pub(crate) fn get_workflow_catalog() -> CommandResult<GetWorkflowCatalogResponse> {
    workspace_setup_read_service()
        .get_workflow_catalog()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_provisioning_profiles() -> CommandResult<GetProvisioningProfilesResponse> {
    workspace_setup_read_service()
        .get_provisioning_profiles()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_endpoint_profiles() -> CommandResult<GetEndpointProfilesResponse> {
    workspace_setup_read_service()
        .get_endpoint_profiles()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_provider_inventory(
    request: GetProviderInventoryRequest,
) -> CommandResult<GetProviderInventoryResponse> {
    let provider_id = request.gpu_cloud_provider_id.into();
    workspace_setup_read_service()
        .get_provider_inventory(provider_id)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_workspace_catalog(
    app: AppHandle,
) -> CommandResult<GetWorkspaceCatalogResponse> {
    let workspace_catalog = sqlite_workspace_catalog(&app).await?;
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderClientRegistry::default(),
        workspace_catalog,
    )
    .get_workspace_catalog()
    .await
    .map(Into::into)
    .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_workspace(
    app: AppHandle,
    request: CreateWorkspaceRequest,
    provider_setup_coordinator: State<'_, ProviderSetupCoordinator>,
) -> CommandResult<CreateWorkspaceResponse> {
    let request: CreateWorkspaceInput = request.try_into().map_err(NativeCommandError::from)?;
    let provider_id = request.gpu_cloud_provider_id;
    let _guard = provider_setup_coordinator.lock(&provider_id).await;

    let workspace_catalog = sqlite_workspace_catalog(&app).await?;
    WorkspaceSetupService::new(
        BundledCatalogReader,
        KeyringSecretStore,
        ProviderClientRegistry::default(),
        workspace_catalog,
    )
    .create_workspace(request)
    .await
    .map(Into::into)
    .map_err(Into::into)
}
