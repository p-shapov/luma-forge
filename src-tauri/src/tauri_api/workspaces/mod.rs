mod errors;

use tauri::State;
use uuid::Uuid;

use errors::{
    CleanupWorkspaceErrorCode, CreateRunpodWorkspaceErrorCode, DeleteWorkspaceErrorCode,
    GetLatestLifecycleOperationErrorCode, GetRunningLifecycleOperationsErrorCode,
    ProvisionWorkspaceErrorCode,
};

use crate::{
    app::state::NativeAppState,
    domain::runpod::RunpodPlacementPlan,
    lifecycle_journal::LifecycleJournalRepository,
    tauri_api::{
        errors::command_error,
        types::workspace::{
            CleanupWorkspaceResponse, CreateRunpodWorkspaceRequest, DeleteWorkspaceResponse,
            LatestLifecycleOperationResponse, ProvisionWorkspaceResponse,
            RunningLifecycleOperationsResponse, WorkspaceIdRequest, WorkspaceResponse,
        },
        CommandResult,
    },
    workspace::{
        CreateRunpodWorkspaceRequest as CreateRunpodWorkspaceServiceRequest, WorkspaceError,
    },
};

#[tauri::command]
#[specta::specta]
pub async fn create_runpod_workspace(
    state: State<'_, NativeAppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse, CreateRunpodWorkspaceErrorCode> {
    const COMMAND: &str = "create_runpod_workspace";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let placement: RunpodPlacementPlan = request.placement.into();
            let workspace_id = Uuid::new_v4().to_string();

            let workspace = state
                .workspace
                .create_runpod_workspace(CreateRunpodWorkspaceServiceRequest {
                    workspace_id: workspace_id.clone(),
                    workflow_preset_id: request.workflow_preset_id,
                    placement,
                })
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        workspace_id = workspace_id.as_str(),
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workspace creation service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(workspace.into())
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
pub async fn provision_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse, ProvisionWorkspaceErrorCode> {
    const COMMAND: &str = "provision_workspace";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let response = state
                .workspace
                .provision_workspace(&request.workspace_id)
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        workspace_id = request.workspace_id.as_str(),
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workspace provision service failed"
                    );
                    command_error(&trace_id, error)
                })?;
            Ok(response.into())
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
pub async fn cleanup_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse, CleanupWorkspaceErrorCode> {
    const COMMAND: &str = "cleanup_workspace";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let response = state
                .workspace
                .cleanup_workspace(&request.workspace_id)
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        workspace_id = request.workspace_id.as_str(),
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workspace cleanup service failed"
                    );
                    command_error(&trace_id, error)
                })?;
            Ok(response.into())
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
pub async fn delete_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse, DeleteWorkspaceErrorCode> {
    const COMMAND: &str = "delete_workspace";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let response = state
                .workspace
                .delete_workspace(&request.workspace_id)
                .await
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        workspace_id = request.workspace_id.as_str(),
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "workspace deletion service failed"
                    );
                    command_error(&trace_id, error)
                })?;

            Ok(response.into())
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
pub async fn get_running_lifecycle_operations(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunningLifecycleOperationsResponse, GetRunningLifecycleOperationsErrorCode> {
    const COMMAND: &str = "get_running_lifecycle_operations";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let operations = state
                .lifecycle_journal
                .list_running()
                .await
                .map_err(WorkspaceError::from)
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "running lifecycle operations lookup service failed"
                    );
                    command_error(&trace_id, error)
                })?
                .into_iter()
                .map(Into::into)
                .collect();

            Ok(RunningLifecycleOperationsResponse { operations })
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
pub async fn get_latest_lifecycle_operation(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse, GetLatestLifecycleOperationErrorCode> {
    const COMMAND: &str = "get_latest_lifecycle_operation";
    super::tracing::run_async_command(COMMAND, |trace_id| async move {
        log::info!(command = COMMAND; "tauri command started");
        let result = async {
            let state = state
                .ready()
                .map_err(|error| command_error(&trace_id, error))?;
            let operation = state
                .lifecycle_journal
                .latest_for_workspace(&request.workspace_id)
                .await
                .map_err(WorkspaceError::from)
                .map_err(|error| {
                    log::error!(
                        command = COMMAND,
                        workspace_id = request.workspace_id.as_str(),
                        error = crate::diagnostics::error_diagnostics_log_json(&error);
                        "latest lifecycle operation lookup service failed"
                    );
                    command_error(&trace_id, error)
                })?
                .map(Into::into);

            Ok(LatestLifecycleOperationResponse { operation })
        }
        .await;
        let status = if result.is_ok() { "ok" } else { "error" };
        log::info!(command = COMMAND, status = status; "tauri command completed");
        result
    })
    .await
}
