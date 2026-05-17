use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    provider_resources::ProviderResourceGateway,
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::context::{SyncStepResult, WorkspaceProvisioningContext};

mod runpod;

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
    match workspace.gpu_cloud_provider_id {
        GpuCloudProviderId::Runpod => runpod::sync(context, workspace).await,
    }
}
