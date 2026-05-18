use crate::{
    domain::workspace::{
        provisioning_state::is_workspace_ready, Workspace, WorkspaceLifecycleState,
    },
    secrets::SecretStore,
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

#[cfg(test)]
mod tests {
    use crate::{
        domain::workspace::{WorkspaceLifecycleState, WorkspaceProvisioningStatus},
        workspace_provisioning::test_support::{ready_provisioning_workspace, TestHarness},
    };

    #[tokio::test]
    async fn sync_marks_workspace_ready_after_endpoint_readiness_criteria() {
        let harness = TestHarness::new(ready_provisioning_workspace());
        let mut workspace = ready_provisioning_workspace();

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("endpoint sync should succeed")
            .expect("ready transition should return result");

        assert_eq!(harness.resources.calls(), vec!["endpoint"]);
        assert_eq!(harness.catalog.updates().len(), 1);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Ready
        );
        assert_eq!(
            result.progress.status,
            WorkspaceProvisioningStatus::Completed
        );
    }
}
