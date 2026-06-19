mod errors;

use tauri::State;
use uuid::Uuid;

use errors::{
    cleanup_workspace_error, create_runpod_workspace_error, delete_workspace_error,
    get_latest_lifecycle_operation_error, get_running_lifecycle_operations_error,
    provision_workspace_error, CleanupWorkspaceErrorCode, CreateRunpodWorkspaceErrorCode,
    DeleteWorkspaceErrorCode, GetLatestLifecycleOperationErrorCode,
    GetRunningLifecycleOperationsErrorCode, ProvisionWorkspaceErrorCode,
};

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
) -> CommandResult<WorkspaceResponse, CreateRunpodWorkspaceErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "create_runpod_workspace",
            error,
            CreateRunpodWorkspaceErrorCode::NativeInitializationFailed,
        )
    })?;
    let placement: RunpodPlacementPlan = request.placement.into();

    let workspace = state
        .workspace
        .create_runpod_workspace(CreateRunpodWorkspaceServiceRequest {
            workspace_id: Uuid::new_v4().to_string(),
            workflow_preset_id: request.workflow_preset_id,
            placement,
        })
        .await
        .map_err(|error| {
            command_error(
                "create_runpod_workspace",
                error,
                create_runpod_workspace_error,
            )
        })?;

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
) -> CommandResult<ProvisionWorkspaceResponse, ProvisionWorkspaceErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "provision_workspace",
            error,
            ProvisionWorkspaceErrorCode::NativeInitializationFailed,
        )
    })?;
    let response = state
        .workspace
        .provision_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("provision_workspace", error, provision_workspace_error))?;
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
) -> CommandResult<CleanupWorkspaceResponse, CleanupWorkspaceErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "cleanup_workspace",
            error,
            CleanupWorkspaceErrorCode::NativeInitializationFailed,
        )
    })?;
    let response = state
        .workspace
        .cleanup_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("cleanup_workspace", error, cleanup_workspace_error))?;
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
) -> CommandResult<DeleteWorkspaceResponse, DeleteWorkspaceErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "delete_workspace",
            error,
            DeleteWorkspaceErrorCode::NativeInitializationFailed,
        )
    })?;
    let response = state
        .workspace
        .delete_workspace(&request.workspace_id)
        .await
        .map_err(|error| command_error("delete_workspace", error, delete_workspace_error))?;

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
) -> CommandResult<RunningLifecycleOperationsResponse, GetRunningLifecycleOperationsErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_running_lifecycle_operations",
            error,
            GetRunningLifecycleOperationsErrorCode::NativeInitializationFailed,
        )
    })?;
    let operations = state
        .lifecycle_journal
        .list_running()
        .await
        .map_err(WorkspaceError::from)
        .map_err(|error| {
            command_error(
                "get_running_lifecycle_operations",
                error,
                get_running_lifecycle_operations_error,
            )
        })?
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
) -> CommandResult<LatestLifecycleOperationResponse, GetLatestLifecycleOperationErrorCode> {
    let state = state.ready().map_err(|error| {
        native_command_error(
            "get_latest_lifecycle_operation",
            error,
            GetLatestLifecycleOperationErrorCode::NativeInitializationFailed,
        )
    })?;
    let operation = state
        .lifecycle_journal
        .latest_for_workspace(&request.workspace_id)
        .await
        .map_err(WorkspaceError::from)
        .map_err(|error| {
            command_error(
                "get_latest_lifecycle_operation",
                error,
                get_latest_lifecycle_operation_error,
            )
        })?
        .map(Into::into);

    Ok(LatestLifecycleOperationResponse { operation })
}
