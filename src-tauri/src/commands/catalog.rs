use tauri::State;

use crate::{
    app::state::AppState,
    commands::{
        types::{
            catalog::{GetProviderPlacementOptionsRequest, WorkflowCatalogResponse},
            placement::RemotePlacementOptionsResponse,
            workspace::WorkspaceCatalogResponse,
        },
        CommandResult,
    },
};

#[tauri::command]
#[specta::specta]
pub fn get_workflow_catalog(state: State<'_, AppState>) -> CommandResult<WorkflowCatalogResponse> {
    let catalog = state.workflow_catalog.get_workflow_catalog()?;

    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_provider_placement_options(
    state: State<'_, AppState>,
    _request: GetProviderPlacementOptionsRequest,
) -> CommandResult<RemotePlacementOptionsResponse> {
    let options = state
        .provisioned_remote
        .get_provider_placement_options()
        .await?;

    Ok(options.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, AppState>,
) -> CommandResult<WorkspaceCatalogResponse> {
    let catalog = state.workspace_catalog.list_workspaces().await?;

    Ok(catalog.into())
}
