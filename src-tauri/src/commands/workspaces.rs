use tauri::State;
use uuid::Uuid;

use crate::{
    app::state::AppState,
    commands::{
        types::workspace::{
            CleanupWorkspaceResponse, CreateWorkspaceRequest, DeleteWorkspaceResponse,
            LatestLifecycleOperationResponse, ProvisionWorkspaceResponse,
            RunningLifecycleOperationsResponse, WorkspaceIdRequest, WorkspaceResponse,
        },
        CommandResult,
    },
    domain::provisioned_remote::RunpodPlacementPlan,
    provisioned_remote::service::CreateProvisionedRemoteWorkspaceRequest,
};

#[tauri::command]
#[specta::specta]
pub async fn create_workspace(
    state: State<'_, AppState>,
    request: CreateWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let remote_placement: RunpodPlacementPlan = request.remote_placement.into();

    let workspace = state
        .provisioned_remote
        .create_workspace(CreateProvisionedRemoteWorkspaceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            remote_placement,
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
        .provisioned_remote
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
        .provisioned_remote
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
        .provisioned_remote
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
        .provisioned_remote
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
        .provisioned_remote
        .get_latest_lifecycle_operation(&request.workspace_id)
        .await?
        .map(Into::into);

    Ok(LatestLifecycleOperationResponse { operation })
}
