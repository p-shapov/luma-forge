use tauri::State;

use crate::{
    app::state::AppState,
    commands::{
        types::{
            catalog::{GetProviderPlacementOptionsRequest, WorkflowCatalogResponse},
            placement::RemotePlacementOptionsResponse,
            workspace::{WorkspaceCatalogResponse, WorkspaceResponse},
        },
        CommandResult, NativeCommandError, NativeCommandErrorCode,
    },
};

#[tauri::command]
#[specta::specta]
pub fn get_workflow_catalog(state: State<'_, AppState>) -> CommandResult<WorkflowCatalogResponse> {
    let catalog = state.workflow_catalog.get_workflow_catalog()?;

    Ok(catalog.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_provider_placement_options(
    state: State<'_, AppState>,
    request: GetProviderPlacementOptionsRequest,
) -> CommandResult<RemotePlacementOptionsResponse> {
    let options = state
        .provisioned_remote
        .get_provider_placement_options(request.provider_id.into())
        .await?;

    Ok(options.into())
}

#[tauri::command]
#[specta::specta]
pub async fn get_workspace_catalog(
    state: State<'_, AppState>,
) -> CommandResult<WorkspaceCatalogResponse> {
    let workflow_catalog = state.workflow_catalog.get_workflow_catalog()?;
    let catalog = state.workspace_catalog.list_workspaces().await?;
    let workspaces = catalog
        .workspaces
        .into_iter()
        .map(|workspace| {
            let workflow = workflow_catalog
                .resolve(&workspace.workflow)
                .ok_or_else(|| {
                    NativeCommandError::new(
                        NativeCommandErrorCode::WorkflowCatalogInvalid,
                        "workspace workflow reference was not found",
                    )
                })?;
            Ok(WorkspaceResponse::from_parts(workspace, workflow))
        })
        .collect::<CommandResult<Vec<_>>>()?;

    Ok(WorkspaceCatalogResponse { workspaces })
}
