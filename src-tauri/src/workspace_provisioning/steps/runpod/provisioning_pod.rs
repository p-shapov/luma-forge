use crate::{
    domain::workspace::Workspace, provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore, workspace_catalog::repository::WorkspaceCatalogRepository,
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
    Ok(context
        .resources
        .sync_provisioning_pod(workspace, &resource_config)
        .await?
        .map(result))
}

pub(crate) async fn finish<S, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    Ok(context
        .resources
        .finish_provisioning_pod(workspace)
        .await?
        .map(result))
}
