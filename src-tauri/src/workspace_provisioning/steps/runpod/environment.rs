use crate::{
    domain::workspace::Workspace,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
    workspace_provisioning::context::{
        SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources,
    },
};

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
    context
        .workspace_provisioner
        .sync_environment(context.workspace_provisioner_context(), workspace)
        .await
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::workspace::{ProviderResourceStatus, WorkspaceLifecycleState},
        secrets::SecretStoreError,
        workspace_provisioner::ProvisionerWorkerError,
        workspace_provisioning::test_support::{
            pod, provisioning_workspace, FakeSecretStore, TestHarness,
        },
    };

    #[tokio::test]
    async fn sync_delegates_to_workspace_provisioner_without_resource_calls() {
        let secrets =
            FakeSecretStore::with_api_key("provider-secret").with_worker_token("worker-secret");
        let harness = TestHarness::with_secrets(provisioning_workspace(), secrets);
        harness
            .workers
            .push_status_result(Err(ProvisionerWorkerError::Unreachable));
        let mut workspace = provisioning_workspace();
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("environment sync should succeed")
            .expect("readiness lag should return progress");

        assert!(harness.resources.calls().is_empty());
        assert_eq!(
            harness.workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
        let serialized = serde_json::to_string(&result.workspace).expect("serialize workspace");
        let progress = serde_json::to_string(&result.progress).expect("serialize progress");
        assert!(!serialized.contains("worker-secret"));
        assert!(!progress.contains("worker-secret"));
    }

    #[tokio::test]
    async fn sync_persists_worker_token_failure_from_workspace_provisioner() {
        let harness = TestHarness::with_secrets(
            provisioning_workspace(),
            FakeSecretStore::with_api_key("provider-secret").with_worker_token_result(Ok(None)),
        );
        let mut workspace = provisioning_workspace();
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("worker failure should persist failed workspace")
            .expect("worker failure should return result");

        assert!(harness.resources.calls().is_empty());
        assert!(harness.workers.status_calls().is_empty());
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
    }

    #[tokio::test]
    async fn sync_propagates_secret_store_infrastructure_error() {
        let harness = TestHarness::with_secrets(
            provisioning_workspace(),
            FakeSecretStore::with_api_key("provider-secret")
                .with_worker_token_result(Err(SecretStoreError::SecureKeyringUnavailable)),
        );
        let mut workspace = provisioning_workspace();
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));

        let error = super::sync(&harness.context(), &mut workspace)
            .await
            .expect_err("secret infrastructure error should propagate");

        assert!(harness.resources.calls().is_empty());
        assert!(harness.workers.status_calls().is_empty());
        assert_eq!(
            error,
            crate::workspace_provisioning::WorkspaceProvisioningError::SecureKeyringUnavailable
        );
    }
}
