use crate::{
    domain::workspace::Workspace, secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
};

use super::super::super::context::{
    SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources,
};
use crate::workspace_provisioning::helpers::result;

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
    let resource_config = context.resource_config();
    Ok(context
        .resources
        .sync_network_volume(workspace, &resource_config)
        .await?
        .map(result))
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::workspace::ProviderResourceStatus,
        workspace_provisioning::test_support::{provisioning_workspace, volume, TestHarness},
        workspace_resources::WorkspaceResourceError,
    };

    #[tokio::test]
    async fn sync_delegates_to_network_volume_resource_step() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut updated = provisioning_workspace();
        updated.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Creating));
        harness
            .resources
            .push_network_volume_result(Ok(Some(updated)));
        let mut workspace = provisioning_workspace();

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("network volume sync should succeed")
            .expect("resource step should return result");

        assert_eq!(harness.resources.calls(), vec!["network_volume"]);
        assert!(result
            .workspace
            .persistent_storage_volume_snapshot
            .is_some());
    }

    #[tokio::test]
    async fn sync_propagates_network_volume_resource_error() {
        let harness = TestHarness::new(provisioning_workspace());
        harness
            .resources
            .push_network_volume_result(Err(WorkspaceResourceError::ProviderRateLimited));
        let mut workspace = provisioning_workspace();

        let error = super::sync(&harness.context(), &mut workspace)
            .await
            .expect_err("network volume error should propagate");

        assert_eq!(harness.resources.calls(), vec!["network_volume"]);
        assert_eq!(
            error,
            crate::workspace_provisioning::WorkspaceProvisioningError::ProviderRateLimited
        );
    }
}
