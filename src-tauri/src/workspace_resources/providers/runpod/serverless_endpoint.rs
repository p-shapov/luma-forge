use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{ProviderResourceStatus, Workspace},
    },
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore},
    workspace_resources::contracts::{
        CreateEndpointTemplateInput, DiscoverEndpointTemplatesInput, EndpointTemplateObservation,
    },
    workspace_resources::{
        state::serverless_endpoint_snapshot, CreateServerlessEndpointInput,
        DiscoverServerlessEndpointsInput, WorkspaceResourceError, WorkspaceResourceOperationResult,
    },
};

use super::{RunPodWorkspaceResourceClient, RunPodWorkspaceResourceContext};

const RUNPOD_SERVERLESS_NETWORK_VOLUME_MOUNT_PATH: &str = "/runpod-volume";

pub(crate) async fn create<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let volume = workspace
        .persistent_storage_volume_snapshot
        .as_ref()
        .cloned()
        .ok_or(WorkspaceResourceError::ProviderResourceNotFound)?;
    let template = ensure_ready_template(context, workspace).await?;
    let Some(template) = template else {
        return Ok(Some(workspace.clone()));
    };
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        endpoint_keep_alive_seconds,
        ..
    } = &workspace.placement_plan;

    let discovered_endpoints = context
        .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if !discovered_endpoints.is_empty() {
        return Err(WorkspaceResourceError::ProviderOrphanedResources);
    }

    let observation = match context
        .create_serverless_endpoint(CreateServerlessEndpointInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            template_id: template.template_id.clone(),
            datacenter_id: selected_datacenter_id.clone(),
            selected_gpu_id: selected_gpu_id.clone(),
            network_volume_id: volume.provider_resource_id,
            endpoint_keep_alive_seconds: *endpoint_keep_alive_seconds,
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
                return Err(WorkspaceResourceError::ProviderOrphanedResources);
            }
            return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
        }
        Err(error) => return Err(error),
    };

    workspace.serverless_endpoint_snapshot = Some(serverless_endpoint_snapshot(
        workspace,
        observation,
        template.template_id,
    ));
    context.update_workspace(workspace).await.map(Some)
}

pub(crate) async fn observe<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &mut Workspace,
) -> WorkspaceResourceOperationResult
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let endpoint_id = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .map(|snapshot| snapshot.provider_resource_id.clone())
        .ok_or(WorkspaceResourceError::ProviderResourceNotFound)?;

    let observation = context
        .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
        .await?;
    let template_id = workspace
        .serverless_endpoint_snapshot
        .as_ref()
        .and_then(|snapshot| {
            snapshot.provider_metadata.as_ref().map(
                |crate::domain::workspace::ServerlessEndpointProviderMetadata::Runpod {
                     template_id,
                 }| template_id.clone(),
            )
        })
        .ok_or(WorkspaceResourceError::ProviderResponseInvalid)?;
    workspace.serverless_endpoint_snapshot = Some(serverless_endpoint_snapshot(
        workspace,
        observation,
        template_id,
    ));
    context.update_workspace(workspace).await.map(Some)
}

async fn ensure_ready_template<S, W, C>(
    context: &RunPodWorkspaceResourceContext<'_, S, W, C>,
    workspace: &Workspace,
) -> Result<Option<EndpointTemplateObservation>, WorkspaceResourceError>
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
    C: RunPodWorkspaceResourceClient,
{
    let discovered_templates = context
        .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
        })
        .await?;
    if let Some(template) = discovered_templates
        .into_iter()
        .find(|template| endpoint_template_matches_workspace(template, workspace))
    {
        return Ok(Some(template));
    }

    let observation = match context
        .create_endpoint_template(CreateEndpointTemplateInput {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
            workspace_id: workspace.id.clone(),
            endpoint_worker_image_ref: workspace.resolved_runtime_image.endpoint_image_ref.clone(),
            mount_path: RUNPOD_SERVERLESS_NETWORK_VOLUME_MOUNT_PATH.to_string(),
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
            if discovered_templates.is_empty() {
                return Err(WorkspaceResourceError::ProviderOperationIndeterminate);
            }
            return discovered_templates
                .into_iter()
                .find(|template| endpoint_template_matches_workspace(template, workspace))
                .map(Some)
                .ok_or(WorkspaceResourceError::ProviderOrphanedResources);
        }
        Err(error) => return Err(error),
    };

    if endpoint_template_matches_workspace(&observation, workspace) {
        Ok(Some(observation))
    } else if matches!(
        observation.provider_resource_status,
        ProviderResourceStatus::Creating
    ) {
        Ok(None)
    } else {
        Err(WorkspaceResourceError::ProviderResponseInvalid)
    }
}

fn endpoint_template_matches_workspace(
    template: &EndpointTemplateObservation,
    workspace: &Workspace,
) -> bool {
    template.provider_resource_status == ProviderResourceStatus::Ready
        && template.endpoint_worker_image_ref == workspace.resolved_runtime_image.endpoint_image_ref
        && template.mount_path == RUNPOD_SERVERLESS_NETWORK_VOLUME_MOUNT_PATH
        && workspace.persistent_storage_volume_snapshot.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_resources::providers::runpod::test_support::*;

    #[tokio::test]
    async fn create_keeps_selected_gpu_for_serverless_endpoint() {
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let client = FakeRunPodClient::default();
        let base_context = context(&secrets, &catalog);
        let runpod_context = RunPodWorkspaceResourceContext::new(&base_context, &client);
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_templates(Ok(vec![runpod_template(
            "template-1",
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/runpod-volume",
        )]));
        client.push_discover_endpoints(Ok(Vec::new()));
        client.push_create_endpoint(Ok(runpod_endpoint(
            "endpoint-1",
            ProviderResourceStatus::Ready,
        )));

        create(&runpod_context, &mut workspace)
            .await
            .expect("serverless endpoint create should succeed");

        let calls = client.calls();
        let RunPodCall::CreateEndpoint(request) = &calls[2] else {
            panic!("expected create endpoint call");
        };
        assert_eq!(request.gpu_type_ids, vec!["gpu-1".to_string()]);
        assert_eq!(request.data_center_ids, vec!["dc-1".to_string()]);
        assert_eq!(request.network_volume_id, "volume-1");
        assert_eq!(request.template_id, "template-1");
    }

    #[tokio::test]
    async fn create_uses_runpod_serverless_network_volume_mount_path_for_template() {
        let secrets = FakeSecretStore::default();
        let catalog = FakeWorkspaceCatalog::default();
        let client = FakeRunPodClient::default();
        let base_context = context(&secrets, &catalog);
        let runpod_context = RunPodWorkspaceResourceContext::new(&base_context, &client);
        let mut workspace = workspace();
        workspace.persistent_storage_volume_snapshot =
            Some(volume_snapshot(ProviderResourceStatus::Ready));
        client.push_discover_templates(Ok(vec![runpod_template(
            "old-template",
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/workspace",
        )]));
        client.push_create_template(Ok(runpod_template(
            "template-1",
            ProviderResourceStatus::Ready,
            "endpoint:latest",
            "/runpod-volume",
        )));
        client.push_discover_endpoints(Ok(Vec::new()));
        client.push_create_endpoint(Ok(runpod_endpoint(
            "endpoint-1",
            ProviderResourceStatus::Ready,
        )));

        create(&runpod_context, &mut workspace)
            .await
            .expect("serverless endpoint create should succeed");

        let calls = client.calls();
        let RunPodCall::CreateTemplate(request) = &calls[1] else {
            panic!("expected create template call");
        };
        assert_eq!(request.volume_mount_path, "/runpod-volume");
        assert_eq!(
            request.env.get("LUMA_FORGE_WORKSPACE_MOUNT_PATH"),
            Some(&"/runpod-volume".to_string())
        );

        let RunPodCall::CreateEndpoint(request) = &calls[3] else {
            panic!("expected create endpoint call");
        };
        assert_eq!(request.template_id, "template-1");
    }
}
