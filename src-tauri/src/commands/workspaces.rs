use tauri::State;
use uuid::Uuid;

use crate::{
    app::state::AppState,
    commands::{
        types::workspace::{CreateWorkspaceRequest, WorkspaceIdRequest, WorkspaceResponse},
        CommandResult, NativeCommandError,
    },
    domain::placement::RemotePlacementPlan,
    remote_workspace::service::SetupWorkspaceRequest,
};

const WORKSPACE_ID_RETRIES: usize = 3;

#[tauri::command]
#[specta::specta]
pub async fn create_workspace(
    state: State<'_, AppState>,
    request: CreateWorkspaceRequest,
) -> CommandResult<WorkspaceResponse> {
    let workflow_catalog = state.workflow_catalog.get_workflow_catalog()?;
    let workflow_preset = workflow_catalog
        .workflow_presets
        .into_iter()
        .find(|preset| preset.id == request.workflow_preset_id)
        .ok_or_else(|| NativeCommandError::new("workflow preset was not found"))?;
    let remote_placement: RemotePlacementPlan = request.remote_placement.into();

    for _ in 0..WORKSPACE_ID_RETRIES {
        let workspace = state
            .remote_workspace
            .setup_workspace(SetupWorkspaceRequest {
                workspace_id: Uuid::new_v4().to_string(),
                workflow_preset: workflow_preset.clone(),
                remote_placement: remote_placement.clone(),
            })?;

        match state.workspace_catalog.insert_workspace(&workspace).await {
            Ok(workspace) => return Ok(workspace.into()),
            Err(crate::workspace_catalog::WorkspaceCatalogError::WorkspaceAlreadyExists) => {}
            Err(error) => return Err(error.into()),
        }
    }

    Err(NativeCommandError::new(
        "workspace id could not be generated",
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn provision_workspace(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<WorkspaceResponse> {
    let workspace = load_workspace(&state, &request.workspace_id).await?;
    let workspace = state
        .remote_workspace
        .provision_workspace(&workspace)
        .await?;
    let workspace = state.workspace_catalog.update_workspace(&workspace).await?;

    Ok(workspace.into())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_workspace_provisioning(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<WorkspaceResponse> {
    let workspace = load_workspace(&state, &request.workspace_id).await?;
    let workspace = state.remote_workspace.cancel_workspace(&workspace)?;
    let workspace = state.workspace_catalog.update_workspace(&workspace).await?;

    Ok(workspace.into())
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_workspace(
    state: State<'_, AppState>,
    request: WorkspaceIdRequest,
) -> CommandResult<WorkspaceResponse> {
    let workspace = load_workspace(&state, &request.workspace_id).await?;
    let workspace = state.remote_workspace.cleanup_workspace(&workspace).await?;
    let workspace = state.workspace_catalog.update_workspace(&workspace).await?;

    Ok(workspace.into())
}

async fn load_workspace(
    state: &State<'_, AppState>,
    workspace_id: &str,
) -> CommandResult<crate::domain::workspace::Workspace> {
    state
        .workspace_catalog
        .find_workspace_by_id(workspace_id)
        .await?
        .ok_or_else(|| NativeCommandError::new("workspace was not found"))
}
