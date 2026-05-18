use crate::{
    domain::workspace::{
        provisioning_state::is_workspace_ready, Workspace, WorkspaceLifecycleState,
    },
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::super::context::{SyncStepResult, WorkspaceProvisioningContext};
use crate::workspace_provisioning::helpers::result;

pub(crate) async fn sync<S, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    let resource_config = context.resource_config();
    if let Some(workspace) = context
        .resources
        .sync_serverless_endpoint(workspace, &resource_config)
        .await?
    {
        return Ok(Some(result(workspace)));
    }

    if is_workspace_ready(workspace) {
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        workspace.last_provisioning_failure = None;
        let workspace = context.update_workspace(workspace).await?;
        return Ok(Some(result(workspace)));
    }

    Ok(None)
}
