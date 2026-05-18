use crate::{
    domain::workspace::Workspace,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
    workspace_provisioning::context::{SyncStepResult, WorkspaceProvisioningContext},
};

pub(crate) async fn sync<S, W, R>(
    context: &WorkspaceProvisioningContext<'_, S, W, R>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    context
        .workspace_provisioner
        .sync_environment(context.workspace_provisioner_context(), workspace)
        .await
}
