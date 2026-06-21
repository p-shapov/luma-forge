mod errors;

use tauri::State;

use errors::{
    get_runpod_placement_options_error, get_runtime_contract_catalog_error,
    get_workflow_catalog_error, get_workspace_catalog_error, GetRunpodPlacementOptionsErrorCode,
    GetRuntimeContractCatalogErrorCode, GetWorkflowCatalogErrorCode, GetWorkspaceCatalogErrorCode,
};

use crate::{
    app::state::NativeAppState,
    runtime_catalog::RuntimeCatalogRepository,
    tauri_api::{
        errors::{command_error, NativeCommandError},
        types::{
            catalog::{RuntimeCatalogResponse, WorkflowCatalogResponse},
            placement::RunpodPlacementOptionsResponse,
            workspace::WorkspaceCatalogResponse,
        },
        CommandResult,
    },
    workflow_catalog::WorkflowCatalogRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

#[tauri::command]
#[specta::specta]
pub fn get_workflow_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkflowCatalogResponse, GetWorkflowCatalogErrorCode> {
    super::tracing::run_sync_command("get_workflow_catalog", |trace_id| {
        let state = state.ready().map_err(|error| {
            command_error(&trace_id, NativeCommandError::from(error), |_| {
                GetWorkflowCatalogErrorCode::NativeInitializationFailed
            })
        })?;
        let catalog = state
            .workflow_catalog
            .get_workflow_catalog()
            .map_err(|error| command_error(&trace_id, error, get_workflow_catalog_error))?;

        Ok(catalog.into())
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_runtime_contract_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<RuntimeCatalogResponse, GetRuntimeContractCatalogErrorCode> {
    super::tracing::run_sync_command("get_runtime_contract_catalog", |trace_id| {
        let state = state.ready().map_err(|error| {
            command_error(&trace_id, NativeCommandError::from(error), |_| {
                GetRuntimeContractCatalogErrorCode::NativeInitializationFailed
            })
        })?;
        let catalog = state
            .runtime_catalog
            .get_runtime_contract_catalog()
            .map_err(|error| command_error(&trace_id, error, get_runtime_contract_catalog_error))?;

        Ok(catalog.into())
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_placement_options(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunpodPlacementOptionsResponse, GetRunpodPlacementOptionsErrorCode> {
    super::tracing::run_async_command("get_runpod_placement_options", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(&trace_id, NativeCommandError::from(error), |_| {
                GetRunpodPlacementOptionsErrorCode::NativeInitializationFailed
            })
        })?;
        let options = state
            .runpod_provider
            .placement_options()
            .await
            .map_err(|error| command_error(&trace_id, error, get_runpod_placement_options_error))?;

        Ok(options.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceCatalogResponse, GetWorkspaceCatalogErrorCode> {
    super::tracing::run_async_command("get_workspace_catalog", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(&trace_id, NativeCommandError::from(error), |_| {
                GetWorkspaceCatalogErrorCode::NativeInitializationFailed
            })
        })?;
        let catalog = state
            .workspace_catalog
            .list_workspaces()
            .await
            .map_err(|error| command_error(&trace_id, error, get_workspace_catalog_error))?;

        Ok(catalog.into())
    })
    .await
}
