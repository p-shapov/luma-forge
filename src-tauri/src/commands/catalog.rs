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
    diagnostics::CommandLogScope,
};

#[tauri::command]
#[specta::specta]
pub fn get_workflow_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkflowCatalogResponse> {
    let command_log = CommandLogScope::new("get_workflow_catalog", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let catalog = state
        .workflow_catalog
        .get_workflow_catalog()
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_placement_options(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunpodPlacementOptionsResponse> {
    let command_log = CommandLogScope::new("get_runpod_placement_options", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let options = state
        .runpod_runtime
        .get_runpod_placement_options()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(options.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceCatalogResponse> {
    let command_log = CommandLogScope::new("get_workspace_catalog", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let catalog = state
        .workspace_catalog
        .list_workspaces()
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(catalog.into())
}
