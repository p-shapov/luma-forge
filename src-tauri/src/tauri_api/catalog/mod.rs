mod errors;

use tauri::State;

use errors::{
    GetRunpodPlacementOptionsErrorCode, GetRuntimeContractCatalogErrorCode,
    GetWorkflowCatalogErrorCode, GetWorkspaceCatalogErrorCode,
};

use crate::{
    app::state::NativeAppState,
    runtime_catalog::RuntimeCatalogRepository,
    tauri_api::{
        errors::command_error,
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
    const COMMAND: &str = "get_workflow_catalog";
    super::tracing::run_sync_command(COMMAND, |trace_id| {
        log::info!(command = COMMAND; "tauri command started");
        let result = {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let catalog = state
                .workflow_catalog
                .get_workflow_catalog()
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workflow catalog service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(catalog.into())
        };
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_runtime_contract_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<RuntimeCatalogResponse, GetRuntimeContractCatalogErrorCode> {
    const COMMAND: &str = "get_runtime_contract_catalog";
    super::tracing::run_sync_command(COMMAND, |trace_id| {
        log::info!(command = COMMAND; "tauri command started");
        let result = {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let catalog = state
                .runtime_catalog
                .get_runtime_contract_catalog()
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "runtime contract catalog service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(catalog.into())
        };
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
}

#[tauri::command]
#[specta::specta]
pub async fn get_runpod_placement_options(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunpodPlacementOptionsResponse, GetRunpodPlacementOptionsErrorCode> {
    const COMMAND: &str = "get_runpod_placement_options";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let options = state
                .runpod_provider
                .placement_options()
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "runpod placement options service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(options.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceCatalogResponse, GetWorkspaceCatalogErrorCode> {
    const COMMAND: &str = "get_workspace_catalog";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let catalog = state
                .workspace_catalog
                .list_workspaces()
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workspace catalog service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(catalog.into())
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}
