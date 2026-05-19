use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::{
        provisioning_state::is_workspace_ready, Workspace, WorkspaceLifecycleState,
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
};

use super::WorkspaceProvisioningProvider;
use crate::workspace_provisioning::{
    context::{SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    helpers::result,
    WorkspaceProvisioningError,
};

#[derive(Debug, Default)]
pub(crate) struct RunPodWorkspaceProvisioningProvider;

impl<S, W, R, Q> WorkspaceProvisioningProvider<S, W, R, Q> for RunPodWorkspaceProvisioningProvider
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    fn sync<'a>(
        &'a self,
        context: &'a WorkspaceProvisioningContext<'_, S, W, R, Q>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = SyncStepResult> + Send + 'a>> {
        Box::pin(async move { sync(context, workspace).await })
    }

    fn cancel<'a>(
        &'a self,
        context: &'a WorkspaceProvisioningContext<'_, S, W, R, Q>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceProvisioningError>> + Send + 'a>>
    {
        Box::pin(async move {
            context
                .resources
                .cleanup_known_resources(workspace)
                .await
                .map_err(Into::into)
        })
    }
}

async fn sync<S, W, R, Q>(
    context: &WorkspaceProvisioningContext<'_, S, W, R, Q>,
    workspace: &mut Workspace,
) -> SyncStepResult
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    if let Some(result) = sync_network_volume(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = sync_provisioning_pod(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = sync_environment(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = finish_provisioning_pod(context, workspace).await? {
        return Ok(Some(result));
    }
    if let Some(result) = sync_endpoint(context, workspace).await? {
        return Ok(Some(result));
    }

    Ok(None)
}

async fn sync_network_volume<S, W, R, Q>(
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

async fn sync_provisioning_pod<S, W, R, Q>(
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

async fn sync_environment<S, W, R, Q>(
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

async fn finish_provisioning_pod<S, W, R, Q>(
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

async fn sync_endpoint<S, W, R, Q>(
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
        domain::workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, WorkspaceLifecycleState,
            WorkspaceProvisioningStatus,
        },
        workspace_provisioner::ProvisionerWorkerError,
        workspace_provisioning::test_support::{
            endpoint, pod, provisioning_workspace, ready_provisioning_workspace, template, volume,
            FakeSecretStore, TestHarness,
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

    #[tokio::test]
    async fn sync_marks_workspace_ready_after_endpoint_readiness_criteria() {
        let harness = TestHarness::new(ready_provisioning_workspace());
        let mut workspace = ready_provisioning_workspace();

        let result = super::sync_endpoint(&harness.context(), &mut workspace)
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
