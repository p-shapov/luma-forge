pub(super) mod contracts;

use crate::{
    app_state::NativeAppState, commands::error::NativeCommandError,
    workspace_setup::contracts::CreateWorkspaceInput,
};
use tauri::State;

use crate::commands::CommandResult;
use contracts::{
    CreateWorkspaceRequest, CreateWorkspaceResponse, GetEndpointProfilesResponse,
    GetProviderInventoryRequest, GetProviderInventoryResponse, GetProvisioningProfilesResponse,
    GetWorkflowCatalogResponse, GetWorkspaceCatalogResponse,
};

#[tauri::command]
#[specta::specta]
pub(crate) fn get_workflow_catalog(
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetWorkflowCatalogResponse> {
    app_state
        .workspace_setup_read_service()
        .get_workflow_catalog()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_provisioning_profiles(
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetProvisioningProfilesResponse> {
    app_state
        .workspace_setup_read_service()
        .get_provisioning_profiles()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_endpoint_profiles(
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetEndpointProfilesResponse> {
    app_state
        .workspace_setup_read_service()
        .get_endpoint_profiles()
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_provider_inventory(
    request: GetProviderInventoryRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetProviderInventoryResponse> {
    let provider_id = request.gpu_cloud_provider_id;
    app_state
        .workspace_setup_read_service()
        .get_provider_inventory(provider_id)
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_workspace_catalog(
    app_state: State<'_, NativeAppState>,
) -> CommandResult<GetWorkspaceCatalogResponse> {
    app_state
        .workspace_setup_service()
        .await
        .map_err(NativeCommandError::from)?
        .get_workspace_catalog()
        .await
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_workspace(
    request: CreateWorkspaceRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<CreateWorkspaceResponse> {
    let request: CreateWorkspaceInput = request.into();
    let provider_id = request.gpu_cloud_provider_id;
    let _guard = app_state
        .provider_setup_coordinator()
        .lock(&provider_id)
        .await;

    app_state
        .workspace_setup_service()
        .await
        .map_err(NativeCommandError::from)?
        .create_workspace(request)
        .await
        .map(Into::into)
        .map_err(Into::into)
}
