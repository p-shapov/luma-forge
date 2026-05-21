use crate::{
    domain::workspace::{ProviderProvisioningSnapshot, Workspace},
    provider::runpod::RunPodClient,
    secrets::AsyncSecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{WorkspaceResourceContext, WorkspaceResourceError},
};

use super::{client::RunPodWorkspaceResourceClient, context::RunPodWorkspaceResourceContext};

pub(super) async fn cleanup_known_resources<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
{
    let client = RunPodClient::default();
    cleanup_known_resources_with_client(&client, context, workspace).await
}

async fn cleanup_known_resources_with_client<S, W, C>(
    client: &C,
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let context = RunPodWorkspaceResourceContext::new(context, client);
    let mut first_error = None;

    if let Some(endpoint) = &workspace.serverless_endpoint_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_serverless_endpoint(
                        workspace.gpu_cloud_provider_id,
                        &endpoint.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(template_id) = runpod_template_id(workspace) {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_endpoint_template(workspace.gpu_cloud_provider_id, &template_id)
                    .await,
            ),
        );
    }

    if let Some(active_pod) = &workspace.active_provisioning_pod_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_provisioning_pod(
                        workspace.gpu_cloud_provider_id,
                        &active_pod.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    if let Some(volume) = &workspace.persistent_storage_volume_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .delete_network_volume(
                        workspace.gpu_cloud_provider_id,
                        &volume.provider_resource_id,
                    )
                    .await,
            ),
        );
    }

    remember_first_error(
        &mut first_error,
        context
            .secrets
            .delete_provisioner_worker_token(&workspace.id)
            .await
            .map_err(WorkspaceResourceError::from),
    );

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn runpod_template_id(workspace: &Workspace) -> Option<String> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(snapshot),
        }) => Some(snapshot.template_id.clone()),
        _ => None,
    }
}

fn tolerate_missing(
    result: Result<(), WorkspaceResourceError>,
) -> Result<(), WorkspaceResourceError> {
    match result {
        Ok(()) | Err(WorkspaceResourceError::ProviderResourceNotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

fn remember_first_error(
    first_error: &mut Option<WorkspaceResourceError>,
    result: Result<(), WorkspaceResourceError>,
) {
    if first_error.is_none() {
        if let Err(error) = result {
            *first_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cleanup_known_resources_with_client;
    use crate::workspace_resources::providers::runpod::test_support::*;
    use crate::{
        domain::workspace::{
            ProviderProvisioningSnapshot, ProviderResourceStatus, RunPodEndpointTemplateSnapshot,
        },
        provider::ProviderClientError,
        secrets::SecretStoreError,
        workspace_resources::WorkspaceResourceError,
    };

    fn workspace_with_all_resources() -> crate::domain::workspace::Workspace {
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot =
            Some(pod_snapshot(ProviderResourceStatus::Running));
        workspace.serverless_endpoint_snapshot =
            Some(endpoint_snapshot(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                template_id: "template-1".to_string(),
                provider_resource_status: ProviderResourceStatus::Ready,
                endpoint_worker_image_ref: "endpoint:latest".to_string(),
                mount_path: "/workspace".to_string(),
            }),
        });
        workspace
    }

    #[tokio::test]
    async fn cleanup_deletes_known_resources_in_dependency_order() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Ok(()));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect("cleanup should succeed");

        let calls = client.calls();
        assert!(matches!(calls[0], RunPodCall::DeleteEndpoint(_)));
        assert!(matches!(calls[1], RunPodCall::DeleteTemplate(_)));
        assert!(matches!(calls[2], RunPodCall::DeletePod(_)));
        assert!(matches!(calls[3], RunPodCall::DeleteNetworkVolume(_)));
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_tolerates_provider_not_found() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Err(ProviderClientError::NotFound));
        client.push_delete_template(Err(ProviderClientError::NotFound));
        client.push_delete_pod(Err(ProviderClientError::NotFound));
        client.push_delete_network_volume(Err(ProviderClientError::NotFound));

        cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect("not found resources should be tolerated");

        assert_eq!(client.calls().len(), 4);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_continues_after_first_real_error_and_returns_it() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Err(ProviderClientError::ApiUnavailable));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        let error = cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect_err("first real provider error should be returned");

        assert_eq!(error, WorkspaceResourceError::ProviderApiUnavailable);
        assert_eq!(client.calls().len(), 4);
        assert_eq!(
            secrets.delete_token_calls(),
            vec!["workspace-1".to_string()]
        );
    }

    #[tokio::test]
    async fn cleanup_returns_token_delete_error_after_provider_cleanup() {
        let client = FakeRunPodClient::default();
        let secrets = FakeSecretStore::default();
        secrets.fail_delete_token(SecretStoreError::SecureKeyringUnavailable);
        let catalog = FakeWorkspaceCatalog::default();
        let context = context(&secrets, &catalog);
        let workspace = workspace_with_all_resources();
        client.push_delete_endpoint(Ok(()));
        client.push_delete_template(Ok(()));
        client.push_delete_pod(Ok(()));
        client.push_delete_network_volume(Ok(()));

        let error = cleanup_known_resources_with_client(&client, &context, &workspace)
            .await
            .expect_err("token delete error should be returned");

        assert_eq!(error, WorkspaceResourceError::SecureKeyringUnavailable);
        assert_eq!(client.calls().len(), 4);
    }
}
