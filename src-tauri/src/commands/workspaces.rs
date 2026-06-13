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
    diagnostics::command_error,
    domain::runpod::RunpodPlacementPlan,
    runpod_runtime::service::CreateRunpodWorkspaceRequest as CreateRunpodWorkspaceServiceRequest,
};

#[tauri::command]
#[specta::specta]
pub async fn create_runpod_workspace(
    state: State<'_, NativeAppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let state = state.ready()?;
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .runpod_runtime
        .create_runpod_workspace(CreateRunpodWorkspaceServiceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            placement,
        })
        .await
        .map_err(|error| command_error("create_runpod_workspace", error))?;

    Ok(workspace.into())
}

#[tauri::command]
#[specta::specta]
pub async fn provision_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse> {
    let state = state.ready()?;
    let response = state
        .runpod_runtime
        .provision_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("provision_workspace", error))?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse> {
    let state = state.ready()?;
    let response = state
        .runpod_runtime
        .cleanup_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("cleanup_workspace", error))?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse> {
    let state = state.ready()?;
    let response = state
        .runpod_runtime
        .delete_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("delete_workspace", error))?;

    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_running_lifecycle_operations(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunningLifecycleOperationsResponse> {
    let state = state.ready()?;
    let operations = state
        .runpod_runtime
        .get_running_lifecycle_operations()
        .await
        .map_err(|error| command_error("get_running_lifecycle_operations", error))?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(RunningLifecycleOperationsResponse { operations })
}

#[tauri::command]
#[specta::specta]
pub async fn get_latest_lifecycle_operation(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse> {
    let state = state.ready()?;
    let operation = state
        .runpod_runtime
        .get_latest_lifecycle_operation(&request.workspace_id)
        .await
        .map_err(|error| command_error("get_latest_lifecycle_operation", error))?
        .map(Into::into);

    Ok(LatestLifecycleOperationResponse { operation })
}
