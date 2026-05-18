use std::collections::HashMap;

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{ProviderProvisioningSnapshot, Workspace},
    },
    provider::runpod::{
        RunPodClient, RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest,
        RunPodCreatePodRequest, RunPodCreateTemplateRequest, RunPodEndpointObservation,
        RunPodNetworkVolumeObservation, RunPodPodObservation, RunPodTemplateObservation,
    },
    secrets::SecretStore,
    workspace_resources::{
        provider_resource_name, CreateEndpointTemplateInput, CreateNetworkVolumeInput,
        CreateProvisioningPodInput, CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput,
        DiscoverNetworkVolumesInput, DiscoverProvisioningPodsInput,
        DiscoverServerlessEndpointsInput, EndpointTemplateObservation, NetworkVolumeObservation,
        ObserveProvisioningPodInput, ProvisioningPodObservation, ServerlessEndpointObservation,
        WorkspaceResourceError,
    },
};

use crate::workspace_resources::{
    WorkspaceResourceConfig, WorkspaceResourceContext, WorkspaceResourceSyncResult,
};

mod network_volume;
mod provisioning_pod;
mod serverless_endpoint;

const RUNPOD_VOLUME_MOUNT_PATH: &str = "/workspace";
const RUNPOD_PROVISIONER_WORKER_HTTP_PORT: u16 = 8000;
const RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT: u16 = 8188;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

pub(crate) async fn sync_network_volume<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    network_volume::sync(context, workspace, config).await
}

pub(crate) async fn sync_provisioning_pod<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    provisioning_pod::sync(context, workspace, config).await
}

pub(crate) async fn finish_provisioning_pod<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    provisioning_pod::finish(context, workspace).await
}

pub(crate) async fn sync_serverless_endpoint<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &mut Workspace,
    config: &WorkspaceResourceConfig,
) -> WorkspaceResourceSyncResult
where
    S: SecretStore,
    W: crate::workspace_catalog::repository::WorkspaceCatalogRepository,
{
    serverless_endpoint::sync(context, workspace, config).await
}

pub(crate) async fn cleanup_known_resources<S, W>(
    context: &WorkspaceResourceContext<'_, S, W>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: SecretStore,
{
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

impl<S, W> WorkspaceResourceContext<'_, S, W>
where
    S: SecretStore,
{
    async fn create_network_volume(
        &self,
        input: CreateNetworkVolumeInput,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .create_network_volume(
                &api_key,
                &RunPodCreateNetworkVolumeRequest {
                    name: provider_resource_name(&input.workspace_id, "volume"),
                    data_center_id: input.datacenter_id,
                    size: bytes_to_gib(input.size_bytes),
                },
            )
            .await
            .map(runpod_network_volume_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .get_network_volume(&api_key, volume_id)
            .await
            .map(runpod_network_volume_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_network_volumes(
        &self,
        input: DiscoverNetworkVolumesInput,
    ) -> Result<Vec<NetworkVolumeObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .find_network_volumes_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "volume"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_network_volume_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .delete_network_volume(&api_key, volume_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_provisioning_pod(
        &self,
        input: CreateProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .create_pod(
                &api_key,
                &RunPodCreatePodRequest {
                    name: provider_resource_name(&input.workspace_id, "provisioner"),
                    image_name: input.provisioner_worker_image_ref,
                    gpu_type_ids: vec![input.selected_gpu_id],
                    data_center_ids: vec![input.datacenter_id],
                    network_volume_id: input.network_volume_id,
                    volume_mount_path: input.mount_path,
                    env: HashMap::from([(
                        "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
                        input.bearer_token.expose_secret().to_string(),
                    )]),
                    ports: vec![format!("{RUNPOD_PROVISIONER_WORKER_HTTP_PORT}/http")],
                },
            )
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_provisioning_pods(
        &self,
        input: DiscoverProvisioningPodsInput,
    ) -> Result<Vec<ProvisioningPodObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .find_pods_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "provisioner"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_pod_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_provisioning_pod(
        &self,
        input: ObserveProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .get_pod(&api_key, &input.provider_resource_id)
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_provisioning_pod(
        &self,
        provider_id: GpuCloudProviderId,
        pod_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .delete_pod(&api_key, pod_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_endpoint_template(
        &self,
        input: CreateEndpointTemplateInput,
    ) -> Result<EndpointTemplateObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .create_template(
                &api_key,
                &RunPodCreateTemplateRequest {
                    name: provider_resource_name(&input.workspace_id, "endpoint-template"),
                    image_name: input.endpoint_worker_image_ref,
                    container_disk_in_gb: 10,
                    env: HashMap::new(),
                    is_public: false,
                    is_serverless: true,
                    ports: vec![format!("{RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT}/http")],
                    readme: String::new(),
                    volume_mount_path: input.mount_path,
                },
            )
            .await
            .map(runpod_template_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_endpoint_templates(
        &self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Result<Vec<EndpointTemplateObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .find_templates_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "endpoint-template"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_template_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_endpoint_template(
        &self,
        provider_id: GpuCloudProviderId,
        template_id: &str,
    ) -> Result<EndpointTemplateObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .get_template(&api_key, template_id)
            .await
            .map(runpod_template_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_endpoint_template(
        &self,
        provider_id: GpuCloudProviderId,
        template_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .delete_template(&api_key, template_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn create_serverless_endpoint(
        &self,
        input: CreateServerlessEndpointInput,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .create_endpoint(
                &api_key,
                &RunPodCreateEndpointRequest {
                    name: provider_resource_name(&input.workspace_id, "endpoint"),
                    template_id: input.template_id,
                    gpu_type_ids: vec![input.selected_gpu_id],
                    network_volume_id: input.network_volume_id,
                    data_center_ids: vec![input.datacenter_id],
                    workers_min: 0,
                    workers_max: 1,
                    scaler_type: "REQUEST_COUNT".to_string(),
                    scaler_value: 1,
                    idle_timeout: input.endpoint_keep_alive_seconds,
                },
            )
            .await
            .map(runpod_endpoint_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn discover_serverless_endpoints(
        &self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Result<Vec<ServerlessEndpointObservation>, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
        RunPodClient::default()
            .find_endpoints_by_name(
                &api_key,
                &provider_resource_name(&input.workspace_id, "endpoint"),
            )
            .await
            .map(|observations| {
                observations
                    .into_iter()
                    .map(runpod_endpoint_observation)
                    .collect()
            })
            .map_err(WorkspaceResourceError::from)
    }

    async fn get_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .get_endpoint(&api_key, endpoint_id)
            .await
            .map(runpod_endpoint_observation)
            .map_err(WorkspaceResourceError::from)
    }

    async fn delete_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id)?;
        RunPodClient::default()
            .delete_endpoint(&api_key, endpoint_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    fn provisioning_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<ProviderApiKey, WorkspaceResourceError> {
        self.secrets
            .read_api_key(provider_id)
            .map_err(WorkspaceResourceError::from)?
            .ok_or(WorkspaceResourceError::ProviderSetupIncomplete)
    }
}

fn runpod_network_volume_observation(
    observation: RunPodNetworkVolumeObservation,
) -> NetworkVolumeObservation {
    NetworkVolumeObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        mount_path: RUNPOD_VOLUME_MOUNT_PATH.to_string(),
    }
}

fn runpod_pod_observation(observation: RunPodPodObservation) -> ProvisioningPodObservation {
    ProvisioningPodObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        provisioner_status_url: observation.provisioner_status_url,
    }
}

fn runpod_template_observation(
    observation: RunPodTemplateObservation,
) -> EndpointTemplateObservation {
    EndpointTemplateObservation {
        template_id: observation.id,
        endpoint_worker_image_ref: observation.image_name,
        mount_path: observation.volume_mount_path,
        provider_resource_status: observation.status,
    }
}

fn runpod_endpoint_observation(
    observation: RunPodEndpointObservation,
) -> ServerlessEndpointObservation {
    ServerlessEndpointObservation {
        provider_resource_id: observation.id,
        provider_resource_status: observation.status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

fn bytes_to_gib(bytes: u64) -> u64 {
    bytes.div_ceil(GIB_BYTES)
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::provider_setup::{GpuCloudProviderId, ProviderApiKey},
        provider::ProviderClientError,
        secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError},
        workspace_resources::{CreateNetworkVolumeInput, WorkspaceResourceError},
    };

    use crate::workspace_catalog::repository::UnavailableWorkspaceCatalog;

    use crate::workspace_resources::WorkspaceResourceContext;

    #[derive(Debug, Clone, Default)]
    struct EmptySecretStore;

    impl SecretStore for EmptySecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            Ok(false)
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            Ok(None)
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            _api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace resource tests do not write secrets")
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace resource tests do not delete secrets")
        }

        fn write_provisioner_worker_token(
            &self,
            _workspace_id: &str,
            _token: &ProvisionerWorkerBearerToken,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace resource tests do not write provisioner tokens")
        }

        fn read_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            unimplemented!("workspace resource tests do not read provisioner tokens")
        }

        fn delete_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace resource tests do not delete provisioner tokens")
        }
    }

    #[test]
    fn provisioning_request_rejection_maps_to_request_rejected() {
        assert_eq!(
            WorkspaceResourceError::from(ProviderClientError::RequestRejected),
            WorkspaceResourceError::ProviderRequestRejected
        );
    }

    #[test]
    fn provisioning_rate_limit_maps_to_rate_limited() {
        assert_eq!(
            WorkspaceResourceError::from(ProviderClientError::RateLimited),
            WorkspaceResourceError::ProviderRateLimited
        );
    }

    #[tokio::test]
    async fn provisioning_fails_before_provider_call_when_setup_missing() {
        let secrets = EmptySecretStore;
        let workspace_catalog = UnavailableWorkspaceCatalog;
        let resources = WorkspaceResourceContext::new(&secrets, &workspace_catalog);

        let error = resources
            .create_network_volume(CreateNetworkVolumeInput {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                size_bytes: 80 * 1024 * 1024 * 1024,
            })
            .await
            .expect_err("missing setup should fail before provider call");

        assert_eq!(error, WorkspaceResourceError::ProviderSetupIncomplete);
    }
}
