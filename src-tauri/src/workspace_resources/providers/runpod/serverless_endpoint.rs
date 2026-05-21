use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
            ServerlessEndpointSnapshot, Workspace, WorkspaceProvisioningPhase,
        },
    },
    secrets::SecretStore,
    workspace_provisioning::{
        failure, failure::fail_workspace, helpers::serverless_endpoint_snapshot,
    },
    workspace_resources::contracts::{
        CreateEndpointTemplateInput, DiscoverEndpointTemplatesInput, EndpointTemplateObservation,
    },
    workspace_resources::{
        state::is_terminal_provider_resource_status, CreateServerlessEndpointInput,
        DiscoverServerlessEndpointsInput, WorkspaceResourceError,
    },
};

use crate::workspace_resources::{WorkspaceResourceConfig, WorkspaceResourceSyncResult};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

fn runpod_template_snapshot(workspace: &Workspace) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

fn endpoint_template_matches_workspace(
    template: &RunPodEndpointTemplateSnapshot,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path
            == workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .map(|volume| volume.mount_path.clone())
                .unwrap_or_default()
}

pub(crate) async fn sync<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    if workspace.environment_prepared_at.is_none()
        || workspace.active_provisioning_pod_snapshot.is_some()
    {
        return Ok(None);
    }

    if let Some(result) = sync_template(context, workspace, config).await? {
        return Ok(Some(result));
    }

    sync_serverless_endpoint(context, workspace).await
}

async fn sync_template<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let template_snapshot = runpod_template_snapshot(workspace);
    if let Some(template) = template_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status == ProviderResourceStatus::Ready)
    {
        if endpoint_template_matches_workspace(template, workspace) {
            return Ok(None);
        }

        match context
            .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
            .await
        {
            Ok(observation) => {
                workspace.provider_provisioning_snapshot =
                    Some(runpod_template_provisioning_snapshot(observation));
                let refreshed_template = runpod_template_snapshot(workspace)
                    .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?;
                if endpoint_template_matches_workspace(&refreshed_template, workspace) {
                    return context.update_workspace(workspace).await.map(Some);
                }
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                match context
                    .delete_endpoint_template(
                        workspace.gpu_cloud_provider_id,
                        &refreshed_template.template_id,
                    )
                    .await
                {
                    Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => {}
                    Err(error) => return Err(error),
                }
                workspace.provider_provisioning_snapshot = None;
                return context.update_workspace(workspace).await.map(Some);
            }
            Err(WorkspaceResourceError::ProviderResourceNotFound) => {
                if let Some(result) = delete_tracked_serverless_endpoint(context, workspace).await?
                {
                    return Ok(Some(result));
                }
                workspace.provider_provisioning_snapshot = None;
                return context.update_workspace(workspace).await.map(Some);
            }
            Err(error) => return Err(error),
        }
    }

    if template_snapshot.is_none() {
        let endpoint_worker_image_ref = workspace.resolved_runtime_image.endpoint_image_ref.clone();
        let discovered_templates = context
            .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_templates.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpoint,
            )
            .await;
        }
        let observation = match context
            .create_endpoint_template(CreateEndpointTemplateInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                endpoint_worker_image_ref: endpoint_worker_image_ref.clone(),
                mount_path: config.volume_mount_path.clone(),
            })
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
                let discovered_templates = context
                    .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_templates.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.provider_provisioning_snapshot =
            Some(runpod_template_provisioning_snapshot(observation));
        fail_if_template_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    let Some(template) = template_snapshot
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
    else {
        return Ok(None);
    };

    let observation = match context
        .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
        .await
    {
        Ok(observation) => observation,
        Err(WorkspaceResourceError::ProviderResourceNotFound) => {
            return fail_for_missing_provider_resource(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpoint,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    workspace.provider_provisioning_snapshot =
        Some(runpod_template_provisioning_snapshot(observation));
    fail_if_template_status_is_terminal(workspace);
    context.update_workspace(workspace).await.map(Some)
}

async fn sync_serverless_endpoint<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    if workspace.serverless_endpoint_snapshot.is_none() {
        let volume = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .cloned();
        let Some(volume) = volume else {
            fail_workspace(
                workspace,
                failure::missing_provider_resource(WorkspaceProvisioningPhase::CreatingEndpoint),
            );
            return context.update_workspace(workspace).await.map(Some);
        };
        let Some(template) = runpod_template_snapshot(workspace) else {
            fail_workspace(
                workspace,
                failure::readiness_validation_failed(WorkspaceProvisioningPhase::CreatingEndpoint),
            );
            return context.update_workspace(workspace).await.map(Some);
        };
        let PlacementPlan::Runpod {
            selected_datacenter_id,
            selected_gpu_id,
            endpoint_keep_alive_seconds,
            ..
        } = &workspace.placement_plan;
        let selected_datacenter_id = selected_datacenter_id.clone();
        let selected_gpu_id = selected_gpu_id.clone();
        let endpoint_keep_alive_seconds = *endpoint_keep_alive_seconds;
        let discovered_endpoints = context
            .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
            })
            .await?;
        if !discovered_endpoints.is_empty() {
            return fail_for_orphaned_provider_resources(
                context,
                workspace,
                WorkspaceProvisioningPhase::CreatingEndpoint,
            )
            .await;
        }
        let observation = match context
            .create_serverless_endpoint(CreateServerlessEndpointInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                workspace_id: workspace.id.clone(),
                template_id: template.template_id.clone(),
                datacenter_id: selected_datacenter_id.clone(),
                selected_gpu_id: selected_gpu_id.clone(),
                network_volume_id: volume.provider_resource_id.clone(),
                endpoint_keep_alive_seconds,
            })
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderOperationIndeterminate) => {
                let discovered_endpoints = context
                    .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                    })
                    .await?;
                if !discovered_endpoints.is_empty() {
                    return fail_for_orphaned_provider_resources(
                        context,
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    )
                    .await;
                }
                return fail_for_indeterminate_provider_operation(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        fail_if_endpoint_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    if let Some(endpoint_id) = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        .map(|snapshot| snapshot.provider_resource_id.clone())
    {
        let observation = match context
            .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceResourceError::ProviderResourceNotFound) => {
                return fail_for_missing_provider_resource(
                    context,
                    workspace,
                    WorkspaceProvisioningPhase::CreatingEndpoint,
                )
                .await;
            }
            Err(error) => return Err(error),
        };
        workspace.serverless_endpoint_snapshot =
            Some(serverless_endpoint_snapshot(workspace, observation));
        fail_if_endpoint_status_is_terminal(workspace);
        return context.update_workspace(workspace).await.map(Some);
    }

    Ok(None)
}

async fn delete_tracked_serverless_endpoint<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let Some(endpoint) = workspace.serverless_endpoint_snapshot.clone() else {
        return Ok(None);
    };

    match context
        .delete_serverless_endpoint(
            workspace.gpu_cloud_provider_id,
            &endpoint.provider_resource_id,
        )
        .await
    {
        Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => {}
        Err(error) => return Err(error),
    }

    workspace.serverless_endpoint_snapshot = None;
    context.update_workspace(workspace).await.map(Some)
}

fn runpod_template_provisioning_snapshot(
    observation: EndpointTemplateObservation,
) -> ProviderProvisioningSnapshot {
    ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: observation.template_id,
            endpoint_worker_image_ref: observation.endpoint_worker_image_ref,
            mount_path: observation.mount_path,
            provider_resource_status: observation.provider_resource_status,
        }),
    }
}

fn _serverless_endpoint_snapshot(
    workspace: &Workspace,
    observation: crate::workspace_resources::ServerlessEndpointObservation,
) -> ServerlessEndpointSnapshot {
    serverless_endpoint_snapshot(workspace, observation)
}

async fn fail_for_indeterminate_provider_operation<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    fail_workspace(workspace, failure::indeterminate_provider_operation(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_missing_provider_resource<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    fail_workspace(workspace, failure::missing_provider_resource(phase));
    context.update_workspace(workspace).await.map(Some)
}

async fn fail_for_orphaned_provider_resources<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
    phase: WorkspaceProvisioningPhase,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    fail_workspace(workspace, failure::orphaned_provider_resources(phase));
    context.update_workspace(workspace).await.map(Some)
}

fn fail_if_template_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = runpod_template_snapshot(workspace)
        .map(|snapshot| snapshot.provider_resource_status)
        .filter(is_terminal_provider_resource_status)
    {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::CreatingEndpoint,
            &status,
        );
        fail_workspace(workspace, failure);
    }
}

fn fail_if_endpoint_status_is_terminal(workspace: &mut Workspace) {
    if let Some(status) = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_status.clone())
        .filter(is_terminal_provider_resource_status)
    {
        let failure = failure::provider_resource_failure(
            WorkspaceProvisioningPhase::CreatingEndpoint,
            &status,
        );
        fail_workspace(workspace, failure);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{sync_serverless_endpoint_with_client, test_support::*};
    use crate::{
        domain::workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
            Workspace, WorkspaceProvisioningFailureCode, WorkspaceProvisioningPhase,
        },
        provider::ProviderClientError,
        workspace_resources::WorkspaceResourceSyncResult,
    };

    async fn sync(
        client: &FakeRunPodClient,
        workspace: &mut Workspace,
        catalog: &FakeWorkspaceCatalog,
    ) -> WorkspaceResourceSyncResult {
        let secrets = FakeSecretStore::default();
        let context = context(&secrets, catalog);
        sync_serverless_endpoint_with_client(client, &context, workspace, &config()).await
    }

    fn prepared_workspace() -> Workspace {
        let mut workspace = workspace();
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        workspace
    }

    fn set_template(
        workspace: &mut Workspace,
        status: ProviderResourceStatus,
        image_ref: &str,
        mount_path: &str,
    ) {
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                template_id: "template-1".to_string(),
                provider_resource_status: status,
                endpoint_worker_image_ref: image_ref.to_string(),
                mount_path: mount_path.to_string(),
            }),
        });
    }

    #[tokio::test]
    async fn waits_until_environment_is_prepared_and_pod_is_gone() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = workspace();
        assert!(sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .is_none());

        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));
        assert!(sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .is_none());
        assert!(client.calls().is_empty());
    }

    #[tokio::test]
    async fn creates_endpoint_template_before_endpoint() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        client.push_discover_templates(Ok(Vec::new()));
        client.push_create_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Creating,
            "endpoint:latest",
            "/workspace",
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert!(updated.serverless_endpoint_snapshot.is_none());
        match updated
            .provider_provisioning_snapshot
            .expect("template snapshot should be stored")
        {
            ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(template),
            } => {
                assert_eq!(template.template_id, "template-1");
                assert_eq!(
                    template.provider_resource_status,
                    ProviderResourceStatus::Creating
                );
            }
            snapshot => panic!("unexpected provisioning snapshot: {snapshot:?}"),
        }
    }

    #[tokio::test]
    async fn refreshes_non_ready_template() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Creating,
            "endpoint:latest",
            "/workspace",
        );
        client.push_get_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        match updated
            .provider_provisioning_snapshot
            .expect("template snapshot should be stored")
        {
            ProviderProvisioningSnapshot::Runpod {
                endpoint_template_snapshot: Some(template),
            } => assert_eq!(
                template.provider_resource_status,
                ProviderResourceStatus::Ready
            ),
            snapshot => panic!("unexpected provisioning snapshot: {snapshot:?}"),
        }
    }

    #[tokio::test]
    async fn missing_or_terminal_template_fails_workspace() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Creating,
            "endpoint:latest",
            "/workspace",
        );
        client.push_get_template(Err(ProviderClientError::NotFound));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderResourceMissing
        );

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Creating,
            "endpoint:latest",
            "/workspace",
        );
        client.push_get_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Failed,
            "endpoint:latest",
            "/workspace",
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .phase,
            WorkspaceProvisioningPhase::CreatingEndpoint
        );
    }

    #[tokio::test]
    async fn orphaned_or_indeterminate_template_create_fails() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        client.push_discover_templates(Ok(vec![runpod_template(
            "orphan-template",
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        )]));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOrphanedResources
        );

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        client.push_discover_templates(Ok(Vec::new()));
        client.push_create_template(Err(ProviderClientError::Indeterminate));
        client.push_discover_templates(Ok(Vec::new()));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
        );
    }

    #[tokio::test]
    async fn stale_template_deletes_endpoint_before_template() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "old:endpoint",
            "/workspace",
        );
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Ready));
        client.push_get_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Ready,
            "old:endpoint",
            "/workspace",
        )));
        client.push_delete_endpoint(Ok(()));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        assert!(updated.serverless_endpoint_snapshot.is_none());
        assert!(matches!(client.calls()[1], RunPodCall::DeleteEndpoint(_)));

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "old:endpoint",
            "/workspace",
        );
        client.push_get_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Ready,
            "old:endpoint",
            "/workspace",
        )));
        client.push_delete_template(Ok(()));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert!(updated.provider_provisioning_snapshot.is_none());
        assert!(matches!(client.calls()[1], RunPodCall::DeleteTemplate(_)));
    }

    #[tokio::test]
    async fn creates_endpoint_from_ready_volume_and_template() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );
        client.push_discover_endpoints(Ok(Vec::new()));
        client.push_create_endpoint(Ok(runpod_endpoint(
            "endpoint-1",
            ProviderResourceStatus::Creating,
        )));

        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");

        let endpoint = updated
            .serverless_endpoint_snapshot
            .expect("endpoint should be stored");
        assert_eq!(endpoint.provider_resource_id, "endpoint-1");
        match &client.calls()[1] {
            RunPodCall::CreateEndpoint(request) => {
                assert_eq!(request.name, "luma-forge-workspace-1-endpoint");
                assert_eq!(request.template_id, "template-1");
                assert_eq!(request.network_volume_id, "volume-1");
            }
            call => panic!("unexpected call: {call:?}"),
        }
    }

    #[tokio::test]
    async fn endpoint_orphan_indeterminate_missing_and_terminal_paths() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );
        client.push_discover_endpoints(Ok(vec![runpod_endpoint(
            "orphan-endpoint",
            ProviderResourceStatus::Ready,
        )]));
        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOrphanedResources
        );

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );
        client.push_discover_endpoints(Ok(Vec::new()));
        client.push_create_endpoint(Err(ProviderClientError::Indeterminate));
        client.push_discover_endpoints(Ok(Vec::new()));
        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
        );

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Creating));
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );
        client.push_get_endpoint(Err(ProviderClientError::NotFound));
        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .code,
            WorkspaceProvisioningFailureCode::ProviderResourceMissing
        );

        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Creating));
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );
        client.push_get_endpoint(Ok(runpod_endpoint(
            "endpoint-1",
            ProviderResourceStatus::Failed,
        )));
        let updated = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed")
            .expect("workspace should be persisted");
        assert_eq!(
            updated
                .last_provisioning_failure
                .expect("workspace should fail")
                .phase,
            WorkspaceProvisioningPhase::CreatingEndpoint
        );
    }

    #[tokio::test]
    async fn ready_endpoint_is_noop() {
        let client = FakeRunPodClient::default();
        let catalog = FakeWorkspaceCatalog::default();
        let mut workspace = prepared_workspace();
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Ready));
        set_template(
            &mut workspace,
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        );

        let result = sync(&client, &mut workspace, &catalog)
            .await
            .expect("sync should succeed");

        assert!(result.is_none());
        assert!(client.calls().is_empty());
        assert!(catalog.updates().is_empty());
    }
}
