use crate::{
    domain::workspace::Workspace, secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
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
        .sync_network_volume(workspace, &resource_config)
        .await?
        .map(result))
}
