use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
};

use super::context::{
    SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources,
};

mod runpod;

pub(crate) async fn sync<S, W, R, Q>(
    context: &WorkspaceProvisioningContext<'_, S, W, R, Q>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    match workspace.gpu_cloud_provider_id {
        GpuCloudProviderId::Runpod => runpod::sync(context, workspace).await,
    }
}
