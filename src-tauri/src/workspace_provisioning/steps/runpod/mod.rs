use crate::{
    domain::workspace::Workspace, secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
};

use super::super::context::{
    SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources,
};

mod endpoint;
mod environment;
mod network_volume;
mod provisioning_pod;

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

#[cfg(test)]
mod tests {
    use crate::{
        domain::workspace::{ProviderProvisioningSnapshot, ProviderResourceStatus},
        workspace_provisioner::ProvisionerWorkerError,
        workspace_provisioning::test_support::{
            endpoint, pod, provisioning_workspace, template, volume, FakeSecretStore, TestHarness,
        },
    };

    #[tokio::test]
    async fn sync_stops_after_network_volume_action() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        let mut updated = provisioning_workspace();
        updated.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Creating));
        harness
            .resources
            .push_network_volume_result(Ok(Some(updated)));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("sync should succeed")
            .expect("volume action should return result");

        assert_eq!(harness.resources.calls(), vec!["network_volume"]);
        assert!(harness.workers.status_calls().is_empty());
        assert!(result
            .workspace
            .persistent_storage_volume_snapshot
            .is_some());
    }

    #[tokio::test]
    async fn sync_stops_after_provisioning_pod_action() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        let mut updated = provisioning_workspace();
        updated.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        updated.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Creating));
        harness
            .resources
            .push_provisioning_pod_result(Ok(Some(updated)));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("sync should succeed")
            .expect("pod action should return result");

        assert_eq!(
            harness.resources.calls(),
            vec!["network_volume", "provisioning_pod"]
        );
        assert!(harness.workers.status_calls().is_empty());
        assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
    }

    #[tokio::test]
    async fn sync_stops_after_environment_progress_before_finish_or_endpoint() {
        let secrets =
            FakeSecretStore::with_api_key("provider-secret").with_worker_token("worker-secret");
        let harness = TestHarness::with_secrets(provisioning_workspace(), secrets);
        harness
            .workers
            .push_status_result(Err(ProvisionerWorkerError::Unreachable));
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("sync should succeed")
            .expect("environment progress should return result");

        assert_eq!(
            harness.resources.calls(),
            vec!["network_volume", "provisioning_pod"]
        );
        assert_eq!(
            harness.secrets.read_worker_token_calls(),
            vec!["workspace-1"]
        );
        assert_eq!(
            harness.workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        let serialized = serde_json::to_string(&result.workspace).expect("serialize workspace");
        let progress = serde_json::to_string(&result.progress).expect("serialize progress");
        assert!(!serialized.contains("worker-secret"));
        assert!(!progress.contains("worker-secret"));
    }

    #[tokio::test]
    async fn sync_stops_after_finish_pod_action() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        let mut updated = provisioning_workspace();
        updated.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        updated.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        updated.last_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Terminated));
        harness.resources.push_finish_pod_result(Ok(Some(updated)));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("sync should succeed")
            .expect("finish action should return result");

        assert_eq!(
            harness.resources.calls(),
            vec!["network_volume", "provisioning_pod", "finish_pod"]
        );
        assert!(harness.workers.status_calls().is_empty());
        assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    }

    #[tokio::test]
    async fn sync_stops_after_endpoint_action() {
        let harness = TestHarness::new(provisioning_workspace());
        let mut workspace = provisioning_workspace();
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        let mut updated = provisioning_workspace();
        updated.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        updated.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
        });
        updated.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        updated.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Creating));
        harness.resources.push_endpoint_result(Ok(Some(updated)));

        let result = super::sync(&harness.context(), &mut workspace)
            .await
            .expect("sync should succeed")
            .expect("endpoint action should return result");

        assert_eq!(
            harness.resources.calls(),
            vec![
                "network_volume",
                "provisioning_pod",
                "finish_pod",
                "endpoint"
            ]
        );
        assert!(result.workspace.serverless_endpoint_snapshot.is_some());
    }
}
