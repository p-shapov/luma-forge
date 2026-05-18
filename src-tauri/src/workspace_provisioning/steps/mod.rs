use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::operations::WorkspaceResourceOperations,
};

use super::context::{SyncStepResult, WorkspaceProvisioningContext};

mod runpod;

pub(crate) async fn sync<S, Q, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, Q, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    Q: WorkspaceResourceOperations,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    match workspace.gpu_cloud_provider_id {
        GpuCloudProviderId::Runpod => runpod::sync(context, workspace).await,
    }
}
