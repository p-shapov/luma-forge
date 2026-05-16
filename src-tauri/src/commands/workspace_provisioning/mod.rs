pub(super) mod contracts;

use tauri::State;

use crate::{
    app_state::NativeAppState,
    commands::{error::NativeCommandError, logging::CommandLog, CommandResult},
};

use contracts::{WorkspaceProvisioningRequest, WorkspaceProvisioningResponse};

#[tauri::command]
#[specta::specta]
pub(crate) async fn initiate_workspace_provisioning(
    request: WorkspaceProvisioningRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceProvisioningResponse> {
    let command_log = CommandLog::new("initiate_workspace_provisioning").start();
    let result = async {
        app_state
            .workspace_provisioning_service()
            .await
            .map_err(NativeCommandError::from)?
            .initiate(&request.workspace_id)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
    .await;
    command_log.finish(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_workspace_provisioning(
    request: WorkspaceProvisioningRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceProvisioningResponse> {
    let command_log = CommandLog::new("sync_workspace_provisioning").start();
    let result = async {
        app_state
            .workspace_provisioning_service()
            .await
            .map_err(NativeCommandError::from)?
            .sync(&request.workspace_id)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
    .await;
    command_log.finish(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_workspace_provisioning(
    request: WorkspaceProvisioningRequest,
    app_state: State<'_, NativeAppState>,
) -> CommandResult<WorkspaceProvisioningResponse> {
    let command_log = CommandLog::new("cancel_workspace_provisioning").start();
    let result = async {
        app_state
            .workspace_provisioning_service()
            .await
            .map_err(NativeCommandError::from)?
            .cancel(&request.workspace_id)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }
    .await;
    command_log.finish(result)
}
