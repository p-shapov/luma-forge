use tauri::State;
use uuid::Uuid;

use crate::{
    app::state::NativeAppState,
    commands::{
        types::workspace::{
            CleanupWorkspaceResponse, CreateRunpodWorkspaceRequest, DeleteWorkspaceResponse,
            LatestLifecycleOperationResponse, ProvisionWorkspaceResponse,
            RunningLifecycleOperationsResponse, WorkspaceIdRequest, WorkspaceResponse,
        },
        CommandResult,
    },
    diagnostics::{command_request_metadata, CommandLogScope},
    domain::runpod::RunpodPlacementPlan,
    runpod_runtime::service::CreateRunpodWorkspaceRequest as CreateRunpodWorkspaceServiceRequest,
};

#[tauri::command]
#[specta::specta]
pub async fn create_runpod_workspace(
    state: State<'_, NativeAppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let command_log = CommandLogScope::new(
        "create_runpod_workspace",
        command_request_metadata(&request),
    );
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .runpod_runtime
        .create_runpod_workspace(CreateRunpodWorkspaceServiceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            placement,
        })
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(workspace.into())
}

#[tauri::command]
#[specta::specta]
pub async fn provision_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse> {
    let command_log =
        CommandLogScope::new("provision_workspace", command_request_metadata(&request));
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let response = state
        .runpod_runtime
        .provision_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_log.failed(error))?;
    command_log.completed();
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse> {
    let command_log = CommandLogScope::new("cleanup_workspace", command_request_metadata(&request));
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let response = state
        .runpod_runtime
        .cleanup_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_log.failed(error))?;
    command_log.completed();
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse> {
    let command_log = CommandLogScope::new("delete_workspace", command_request_metadata(&request));
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let response = state
        .runpod_runtime
        .delete_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_log.failed(error))?;

    command_log.completed();
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_running_lifecycle_operations(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunningLifecycleOperationsResponse> {
    let command_log = CommandLogScope::new("get_running_lifecycle_operations", Vec::new());
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let operations = state
        .runpod_runtime
        .get_running_lifecycle_operations()
        .await
        .map_err(|error| command_log.failed(error))?
        .into_iter()
        .map(Into::into)
        .collect();

    command_log.completed();
    Ok(RunningLifecycleOperationsResponse { operations })
}

#[tauri::command]
#[specta::specta]
pub async fn get_latest_lifecycle_operation(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse> {
    let command_log = CommandLogScope::new(
        "get_latest_lifecycle_operation",
        command_request_metadata(&request),
    );
    let state = state
        .ready()
        .map_err(|error| command_log.failed_native(error))?;
    let operation = state
        .runpod_runtime
        .get_latest_lifecycle_operation(&request.workspace_id)
        .await
        .map_err(|error| command_log.failed(error))?
        .map(Into::into);

    command_log.completed();
    Ok(LatestLifecycleOperationResponse { operation })
}
