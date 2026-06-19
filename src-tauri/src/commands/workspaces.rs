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
    diagnostics::{
        command_error, command_request_metadata, empty_command_request_metadata,
        native_command_error,
    },
    domain::runpod::RunpodPlacementPlan,
    lifecycle_journal::LifecycleJournalRepository,
    workspace::{
        CreateRunpodWorkspaceRequest as CreateRunpodWorkspaceServiceRequest, WorkspaceError,
    },
};

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "create_runpod_workspace", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn create_runpod_workspace(
    state: State<'_, NativeAppState>,
    request: CreateRunpodWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .workspace
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
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "provision_workspace", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn provision_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let response = state
        .workspace
        .provision_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("provision_workspace", error))?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "cleanup_workspace", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn cleanup_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let response = state
        .workspace
        .cleanup_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("cleanup_workspace", error))?;
    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "delete_workspace", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn delete_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let response = state
        .workspace
        .delete_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("delete_workspace", error))?;

    Ok(response.into())
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_running_lifecycle_operations", request_metadata = tracing::field::debug(empty_command_request_metadata()))
)]
pub async fn get_running_lifecycle_operations(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunningLifecycleOperationsResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let operations = state
        .lifecycle_journal
        .list_running()
        .await
        .map_err(WorkspaceError::from)
        .map_err(|error| command_error("get_running_lifecycle_operations", error))?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(RunningLifecycleOperationsResponse { operations })
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(
    name = "native_command",
    skip_all,
    fields(command = "get_latest_lifecycle_operation", request_metadata = tracing::field::debug(command_request_metadata(&request)))
)]
pub async fn get_latest_lifecycle_operation(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse> {
    let state = state.ready().map_err(native_command_error)?;
    let operation = state
        .lifecycle_journal
        .latest_for_workspace(&request.workspace_id)
        .await
        .map_err(WorkspaceError::from)
        .map_err(|error| command_error("get_latest_lifecycle_operation", error))?
        .map(Into::into);

    Ok(LatestLifecycleOperationResponse { operation })
}
