use crate::{
    domain::workspace::Workspace,
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    context::{SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    failure,
    gateway::ProvisionerWorkerGateway,
    helpers::result,
    WorkspaceProvisioningError,
};

pub(crate) async fn sync_hugging_face_api_key_setup<S, W, R, Q>(
    context: &WorkspaceProvisioningContext<'_, S, W, R, Q>,
    workspace: &mut Workspace,
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
        || context
            .secrets
            .has_hugging_face_api_key_entry()
            .await
            .map_err(WorkspaceProvisioningError::from)?
    {
        return Ok(None);
    }

    let mut workspace = workspace.clone();
    failure::fail_workspace(
        &mut workspace,
        failure::hugging_face_api_key_setup_required(),
    );
    let workspace = context.update_workspace(&workspace).await?;
    Ok(Some(result(workspace)))
}
