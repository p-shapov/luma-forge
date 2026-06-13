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
    diagnostics::{command_error, empty_command_request_metadata, native_command_error},
};

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_workflow_catalog", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub fn get_workflow_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkflowCatalogResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let catalog = state
        .workflow_catalog
        .get_workflow_catalog()
        .map_err(|error| command_error("get_workflow_catalog", error))?;

    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_runpod_placement_options", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn get_runpod_placement_options(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunpodPlacementOptionsResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let options = state
        .runpod_runtime
        .get_runpod_placement_options()
        .await
        .map_err(|error| command_error("get_runpod_placement_options", error))?;

    Ok(options.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_workspace_catalog", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn get_workspace_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceCatalogResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let catalog = state
        .workspace_catalog
        .list_workspaces()
        .await
        .map_err(|error| command_error("get_workspace_catalog", error))?;

    Ok(catalog.into())
}
