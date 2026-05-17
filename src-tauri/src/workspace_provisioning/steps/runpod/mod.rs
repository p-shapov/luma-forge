use crate::{
    domain::workspace::Workspace, provider_resources::ProviderResourceGateway,
    provisioner_worker::ProvisionerWorkerGateway, secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::super::context::{SyncStepResult, WorkspaceProvisioningContext};

mod endpoint;
mod environment;
mod network_volume;
mod provisioning_pod;

pub(crate) async fn sync<S, P, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, P, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    P: ProviderResourceGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    if let Some(result) = network_volume::sync(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = provisioning_pod::sync(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = environment::sync(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = provisioning_pod::finish(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = endpoint::sync(context, workspace).await? {
        return Ok(Some(result));
    }

    Ok(None)
}
