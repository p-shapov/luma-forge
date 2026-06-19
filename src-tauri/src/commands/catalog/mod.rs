mod errors;

use tauri::State;

use errors::{
    get_runpod_placement_options_error, get_runtime_contract_catalog_error,
    get_workflow_catalog_error, get_workspace_catalog_error, GetRunpodPlacementOptionsErrorCode,
    GetRuntimeContractCatalogErrorCode, GetWorkflowCatalogErrorCode, GetWorkspaceCatalogErrorCode,
};

use crate::{
    app::state::NativeAppState,
    commands::{
        types::{
            catalog::{RuntimeCatalogResponse, WorkflowCatalogResponse},
            placement::RunpodPlacementOptionsResponse,
            workspace::WorkspaceCatalogResponse,
        },
        CommandResult,
    },
    diagnostics::{command_error, empty_command_request_metadata, native_command_error},
    runtime_catalog::RuntimeCatalogRepository,
    workflow_catalog::WorkflowCatalogRepository,
    workspace_catalog::WorkspaceCatalogRepository,
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
) -> CommandResult<WorkflowCatalogResponse, GetWorkflowCatalogErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_workflow_catalog",
            error,
            GetWorkflowCatalogErrorCode::NativeInitializationFailed,
        )
    })?;
    let catalog = state
        .workflow_catalog
        .get_workflow_catalog()
        .map_err(|error| {
            command_error("get_workflow_catalog", error, get_workflow_catalog_error)
        })?;

    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_runtime_contract_catalog", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub fn get_runtime_contract_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<RuntimeCatalogResponse, GetRuntimeContractCatalogErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_runtime_contract_catalog",
            error,
            GetRuntimeContractCatalogErrorCode::NativeInitializationFailed,
        )
    })?;
    let catalog = state
        .runtime_catalog
        .get_runtime_contract_catalog()
        .map_err(|error| {
            command_error(
                "get_runtime_contract_catalog",
                error,
                get_runtime_contract_catalog_error,
            )
        })?;

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
) -> CommandResult<RunpodPlacementOptionsResponse, GetRunpodPlacementOptionsErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_runpod_placement_options",
            error,
            GetRunpodPlacementOptionsErrorCode::NativeInitializationFailed,
        )
    })?;
    let options = state
        .runpod_provider
        .placement_options()
        .await
        .map_err(|error| {
            command_error(
                "get_runpod_placement_options",
                error,
                get_runpod_placement_options_error,
            )
        })?;

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
) -> CommandResult<WorkspaceCatalogResponse, GetWorkspaceCatalogErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_workspace_catalog",
            error,
            GetWorkspaceCatalogErrorCode::NativeInitializationFailed,
        )
    })?;
    let catalog = state
        .workspace_catalog
        .list_workspaces()
        .await
        .map_err(|error| {
            command_error("get_workspace_catalog", error, get_workspace_catalog_error)
        })?;

    Ok(catalog.into())
}
