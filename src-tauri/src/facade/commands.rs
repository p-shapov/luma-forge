use super::*;

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_workflows(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: PageRequest,
) -> Result<WorkflowPageDto, CommandError<GetWorkflowsErrorCode>> {
    state.get_workflows(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_workspaces(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: PageRequest,
) -> Result<WorkspacePageDto, CommandError<GetWorkspacesErrorCode>> {
    state.get_workspaces(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn create_workspace(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: CreateWorkspaceRequest,
) -> Result<WorkspaceDto, CommandError<CreateWorkspaceErrorCode>> {
    state.create_workspace(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn delete_workspace(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: WorkspaceIdRequest,
) -> Result<(), CommandError<DeleteWorkspaceErrorCode>> {
    state.delete_workspace(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn provision_workspace(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: ProvisionWorkspaceRequest,
) -> Result<WorkspaceOperationDto, CommandError<ProvisionWorkspaceErrorCode>> {
    state.provision_workspace(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn cleanup_workspace(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: WorkspaceIdRequest,
) -> Result<WorkspaceOperationDto, CommandError<CleanupWorkspaceErrorCode>> {
    state.cleanup_workspace(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_runtime_operations(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: RuntimeOperationPageRequest,
) -> Result<RuntimeOperationPageDto, CommandError<GetRuntimeOperationsErrorCode>> {
    state.get_runtime_operations(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_runpod_placement(
    state: tauri::State<'_, FacadeState>,
) -> Result<RunpodPlacementDto, CommandError<GetRunpodPlacementErrorCode>> {
    state.get_runpod_placement().await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn setup_runpod_api_key(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: SetupApiKeyRequest,
) -> Result<IdentityDto, CommandError<SetupApiKeyErrorCode>> {
    state.setup_runpod_api_key(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn setup_hugging_face_api_key(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: SetupApiKeyRequest,
) -> Result<IdentityDto, CommandError<SetupApiKeyErrorCode>> {
    state.setup_hugging_face_api_key(request).await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_runpod_identity(
    state: tauri::State<'_, FacadeState>,
) -> Result<IdentityDto, CommandError<GetIdentityErrorCode>> {
    state.get_runpod_identity().await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_hugging_face_identity(
    state: tauri::State<'_, FacadeState>,
) -> Result<IdentityDto, CommandError<GetIdentityErrorCode>> {
    state.get_hugging_face_identity().await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn delete_runpod_api_key(
    state: tauri::State<'_, FacadeState>,
) -> Result<(), CommandError<DeleteApiKeyErrorCode>> {
    state.delete_runpod_api_key().await
}

#[tauri::command]
#[specta::specta]
#[luma_diagnostics::diagnostic(root, show_output, show_error)]
pub async fn delete_hugging_face_api_key(
    state: tauri::State<'_, FacadeState>,
) -> Result<(), CommandError<DeleteApiKeyErrorCode>> {
    state.delete_hugging_face_api_key().await
}
