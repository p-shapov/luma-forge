use tauri::State;
use uuid::Uuid;

use crate::{
    app::state::AppState,
    commands::{
        types::workspace::{
            CleanupWorkspaceResponse, CreateRunpodWorkspaceRequest, DeleteWorkspaceResponse,
            LatestLifecycleOperationResponse, ProvisionWorkspaceResponse,
            RunningLifecycleOperationsResponse, WorkspaceIdRequest, WorkspaceResponse,
        },
        CommandResult,
    },
    domain::provisioned_remote::RunpodPlacementPlan,
    provisioned_remote::service::CreateRunpodWorkspaceRequest as CreateRunpodWorkspaceServiceRequest,
};

#[tauri::command]
#[specta::specta]
pub async fn create_runpod_workspace(
    state: State<'_, AppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .runpod_runtime
        .create_runpod_workspace(CreateRunpodWorkspaceServiceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            placement,
        })
        .await?;

    Ok(workspace.into())
}

#[tauri::command]
#[specta::specta]
pub async fn provision_workspace(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse> {
    let response = state
        .runpod_runtime
        .provision_workspace(&request.workspace_id)
        .await?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_workspace(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse> {
    let response = state
        .runpod_runtime
        .cleanup_workspace(&request.workspace_id)
        .await?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_workspace(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse> {
    let response = state
        .runpod_runtime
        .delete_workspace(&request.workspace_id)
        .await?;

    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_running_lifecycle_operations(
    state: State<'_, AppState>,
) -> CommandResult<RunningLifecycleOperationsResponse> {
    let operations = state
        .runpod_runtime
        .get_running_lifecycle_operations()
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(RunningLifecycleOperationsResponse { operations })
}

#[tauri::command]
#[specta::specta]
pub async fn get_latest_lifecycle_operation(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse> {
    let operation = state
        .runpod_runtime
        .get_latest_lifecycle_operation(&request.workspace_id)
        .await?
        .map(Into::into);

    Ok(LatestLifecycleOperationResponse { operation })
}
