use crate::{
    domain::workspace::{Workspace, WorkspaceProvisioningPhase},
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProvisionerTokenStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    context::{SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    failure,
    gateway::ProvisionerWorkerGateway,
    helpers::result,
};

pub(crate) async fn sync_hugging_face_api_key_setup<S, W, R, Q>(
    context: &WorkspaceProvisioningContext<'_, S, W, R, Q>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> SyncStepResult
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    if !workspace
        .placement_plan
        .selected_workflow_preset()
        .requires_hugging_face_api_key
    {
        return Ok(None);
    }

    match context.secrets.read_hugging_face_api_key().await {
        Ok(Some(_)) => return Ok(None),
        Ok(None) | Err(SecretStoreError::InvalidStoredHuggingFaceApiKey) => {}
        Err(error) => return Err(error.into()),
    }

    let mut workspace = workspace.clone();
    failure::fail_workspace(
        &mut workspace,
        failure::hugging_face_api_key_setup_required(phase),
    );
    let workspace = context.update_workspace(&workspace).await?;
    Ok(Some(result(workspace)))
}
