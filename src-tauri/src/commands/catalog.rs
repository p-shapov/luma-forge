use tauri::State;

use crate::{
    app::state::NativeAppState,
    commands::{
        types::{
            catalog::WorkflowCatalogResponse, placement::RunpodPlacementOptionsResponse,
            workspace::WorkspaceCatalogResponse,
        },
        CommandResult,
    },
};

#[tauri::command]
#[specta::specta]
pub fn get_workflow_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkflowCatalogResponse> {
    let state = state.ready()?;
    let catalog = state.workflow_catalog.get_workflow_catalog()?;

    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_placement_options(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunpodPlacementOptionsResponse> {
    let state = state.ready()?;
    let options = state.runpod_runtime.get_runpod_placement_options().await?;

    Ok(options.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceCatalogResponse> {
    let state = state.ready()?;
    let catalog = state.workspace_catalog.list_workspaces().await?;

    Ok(catalog.into())
}
