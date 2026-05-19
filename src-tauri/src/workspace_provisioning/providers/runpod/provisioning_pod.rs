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
        .sync_provisioning_pod(workspace, &resource_config)
        .await?
        .map(result))
}

pub(crate) async fn finish<S, W, R, Q>(
    context: &WorkspaceProvisioningContext<'_, S, W, R, Q>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    Ok(context
        .resources
        .finish_provisioning_pod(workspace)
        .await?
        .map(result))
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::workspace::ProviderResourceStatus,
        workspace_provisioning::test_support::{pod, provisioning_workspace, volume, TestHarness},
        workspace_resources::WorkspaceResourceError,
    };

    #[tokio::test]
    async fn sync_delegates_to_provisioning_pod_resource_step() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        let mut updated = workspace.clone();
        updated.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Creating));
        harness
            .resources
            .push_provisioning_pod_result(Ok(Some(updated)));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("pod sync should succeed")
            .expect("resource step should return result");

        assert_eq!(harness.resources.calls(), vec!["provisioning_pod"]);
        assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
    }

    #[tokio::test]
    async fn sync_propagates_provisioning_pod_resource_error() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        harness
            .resources
            .push_provisioning_pod_result(Err(WorkspaceResourceError::ProviderApiUnavailable));

        let error = super::sync(&harness.context(), &mut workspace)
            .await
            .expect_err("pod sync error should propagate");

        assert_eq!(harness.resources.calls(), vec!["provisioning_pod"]);
        assert_eq!(
            error,
            crate::workspace_provisioning::WorkspaceProvisioningError::ProviderApiUnavailable
        );
    }

    #[tokio::test]
    async fn finish_delegates_to_finish_pod_resource_step() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        let mut updated = workspace.clone();
        updated.active_provisioning_pod_snapshot = None;
        updated.last_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Terminated));
        harness.resources.push_finish_pod_result(Ok(Some(updated)));

        let result = super::finish(&harness.context(), &mut workspace)
            .await
            .expect("finish pod should succeed")
            .expect("resource step should return result");

        assert_eq!(harness.resources.calls(), vec!["finish_pod"]);
        assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
        assert!(result.workspace.last_provisioning_pod_snapshot.is_some());
    }

    #[tokio::test]
    async fn finish_propagates_resource_error() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        harness
            .resources
            .push_finish_pod_result(Err(WorkspaceResourceError::SecureKeyringUnavailable));

        let error = super::finish(&harness.context(), &mut workspace)
            .await
            .expect_err("finish pod error should propagate");

        assert_eq!(harness.resources.calls(), vec!["finish_pod"]);
        assert_eq!(
            error,
            crate::workspace_provisioning::WorkspaceProvisioningError::SecureKeyringUnavailable
        );
    }
}
