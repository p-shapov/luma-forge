mod errors;

use tauri::State;
use uuid::Uuid;

use errors::{
    CleanupWorkspaceCommandError, CleanupWorkspaceErrorCode, CreateRunpodWorkspaceCommandError,
    CreateRunpodWorkspaceErrorCode, DeleteWorkspaceCommandError, DeleteWorkspaceErrorCode,
    GetLatestLifecycleOperationCommandError, GetLatestLifecycleOperationErrorCode,
    GetRunningLifecycleOperationsCommandError, GetRunningLifecycleOperationsErrorCode,
    ProvisionWorkspaceCommandError, ProvisionWorkspaceErrorCode,
};

use crate::{
    app::state::NativeAppState,
    domain::runpod::RunpodPlacementPlan,
    lifecycle_journal::LifecycleJournalRepository,
    tauri_api::{
        errors::{command_error, NativeInitializationCommandError},
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
    super::tracing::run_async_command("create_runpod_workspace", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                CreateRunpodWorkspaceCommandError::from(NativeInitializationCommandError::from(
                    error,
                )),
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
                command_error(&trace_id, CreateRunpodWorkspaceCommandError::from(error))
            })?;

        Ok(workspace.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn provision_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<ProvisionWorkspaceResponse, ProvisionWorkspaceErrorCode> {
    super::tracing::run_async_command("provision_workspace", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                ProvisionWorkspaceCommandError::from(NativeInitializationCommandError::from(error)),
            )
        })?;
        let response = state
            .workspace
            .provision_workspace(&request.workspace_id)
            .await
            .map_err(|error| {
                command_error(&trace_id, ProvisionWorkspaceCommandError::from(error))
            })?;
        Ok(response.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<CleanupWorkspaceResponse, CleanupWorkspaceErrorCode> {
    super::tracing::run_async_command("cleanup_workspace", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                CleanupWorkspaceCommandError::from(NativeInitializationCommandError::from(error)),
            )
        })?;
        let response = state
            .workspace
            .cleanup_workspace(&request.workspace_id)
            .await
            .map_err(|error| command_error(&trace_id, CleanupWorkspaceCommandError::from(error)))?;
        Ok(response.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_workspace(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<DeleteWorkspaceResponse, DeleteWorkspaceErrorCode> {
    super::tracing::run_async_command("delete_workspace", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                DeleteWorkspaceCommandError::from(NativeInitializationCommandError::from(error)),
            )
        })?;
        let response = state
            .workspace
            .delete_workspace(&request.workspace_id)
            .await
            .map_err(|error| command_error(&trace_id, DeleteWorkspaceCommandError::from(error)))?;

        Ok(response.into())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_running_lifecycle_operations(
    state: State<'_, NativeAppState>,
) -> CommandResult<RunningLifecycleOperationsResponse, GetRunningLifecycleOperationsErrorCode> {
    super::tracing::run_async_command("get_running_lifecycle_operations", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                GetRunningLifecycleOperationsCommandError::from(
                    NativeInitializationCommandError::from(error),
                ),
            )
        })?;
        let operations = state
            .lifecycle_journal
            .list_running()
            .await
            .map_err(WorkspaceError::from)
            .map_err(|error| {
                command_error(
                    &trace_id,
                    GetRunningLifecycleOperationsCommandError::from(error),
                )
            })?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(RunningLifecycleOperationsResponse { operations })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_latest_lifecycle_operation(
    state: State<'_, NativeAppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<LatestLifecycleOperationResponse, GetLatestLifecycleOperationErrorCode> {
    super::tracing::run_async_command("get_latest_lifecycle_operation", |trace_id| async move {
        let state = state.ready().map_err(|error| {
            command_error(
                &trace_id,
                GetLatestLifecycleOperationCommandError::from(
                    NativeInitializationCommandError::from(error),
                ),
            )
        })?;
        let operation = state
            .lifecycle_journal
            .latest_for_workspace(&request.workspace_id)
            .await
            .map_err(WorkspaceError::from)
            .map_err(|error| {
                command_error(
                    &trace_id,
                    GetLatestLifecycleOperationCommandError::from(error),
                )
            })?
            .map(Into::into);

        Ok(LatestLifecycleOperationResponse { operation })
    })
    .await
}
