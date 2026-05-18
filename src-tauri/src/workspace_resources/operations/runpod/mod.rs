use std::{collections::HashMap, future::Future, pin::Pin};

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{
            provisioning_state::reset_after_resource_cleanup, ProviderProvisioningSnapshot,
            Workspace,
        },
    },
    provider::runpod::{
        RunPodClient, RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest,
        RunPodCreatePodRequest, RunPodCreateTemplateRequest, RunPodEndpointObservation,
        RunPodNetworkVolumeObservation, RunPodPodObservation, RunPodTemplateObservation,
    },
    secrets::{KeyringSecretStore, SecretStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        provider_resource_name, CreateEndpointTemplateInput, CreateNetworkVolumeInput,
        CreateProvisioningPodInput, CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput,
        DiscoverNetworkVolumesInput, DiscoverProvisioningPodsInput,
        DiscoverServerlessEndpointsInput, EndpointTemplateObservation, NetworkVolumeObservation,
        ObserveProvisioningPodInput, ProvisioningPodObservation, ServerlessEndpointObservation,
        WorkspaceResourceError,
    },
};

use super::{WorkspaceResourceConfig, WorkspaceResourceOperations, WorkspaceResourceSyncResult};

mod network_volume;
mod provisioning_pod;
mod serverless_endpoint;

const RUNPOD_VOLUME_MOUNT_PATH: &str = "/workspace";
const RUNPOD_PROVISIONER_WORKER_HTTP_PORT: u16 = 8000;
const RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT: u16 = 8188;
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct RunPodWorkspaceResourceOperations<S, W, G = RunPodResourceClient<S>> {
    pub(crate) secrets: S,
    pub(crate) workspace_catalog: W,
    pub(crate) resources: G,
}

impl<S, W> RunPodWorkspaceResourceOperations<S, W, RunPodResourceClient<S>>
where
    S: Clone,
{
    pub(crate) fn production(secrets: S, workspace_catalog: W, runpod: RunPodClient) -> Self {
        Self::new(
            secrets.clone(),
            workspace_catalog,
            RunPodResourceClient::new(secrets, runpod),
        )
    }
}

impl<S, W, G> RunPodWorkspaceResourceOperations<S, W, G> {
    pub(crate) fn new(secrets: S, workspace_catalog: W, resources: G) -> Self {
        Self {
            secrets,
            workspace_catalog,
            resources,
        }
    }
}

impl<S, W, G> WorkspaceResourceOperations for RunPodWorkspaceResourceOperations<S, W, G>
where
    S: SecretStore,
    W: WorkspaceCatalogRepository,
    G: RunPodResourceGateway,
{
    fn sync_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(network_volume::sync(self, workspace, config))
    }

    fn sync_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(provisioning_pod::sync(self, workspace, config))
    }

    fn finish_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(provisioning_pod::finish(self, workspace))
    }

    fn sync_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(serverless_endpoint::sync(self, workspace, config))
    }

    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            cleanup_known_resources(self, workspace).await?;
            reset_after_resource_cleanup(workspace);
            self.update_workspace(workspace).await
        })
    }
}

impl<S, W, G> RunPodWorkspaceResourceOperations<S, W, G>
where
    W: WorkspaceCatalogRepository,
{
    pub(crate) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(WorkspaceResourceError::from)
    }
}

async fn cleanup_known_resources<S, W, G>(
    context: &RunPodWorkspaceResourceOperations<S, W, G>,
    workspace: &Workspace,
) -> Result<(), WorkspaceResourceError>
where
    S: SecretStore,
    G: RunPodResourceGateway,
{
    let mut first_error = None;

    if let Some(endpoint) = &workspace.serverless_endpoint_snapshot {
        remember_first_error(
            &mut first_error,
            tolerate_missing(
                context
                    .resources
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
                    .resources
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
                    .resources
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
                    .resources
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

#[derive(Debug, Clone)]
pub(crate) struct RunPodResourceClient<S = KeyringSecretStore> {
    secrets: S,
    runpod: RunPodClient,
}

impl<S> RunPodResourceClient<S> {
    pub(crate) fn new(secrets: S, runpod: RunPodClient) -> Self {
        Self { secrets, runpod }
    }
}

pub(crate) trait RunPodResourceGateway: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_network_volumes<'a>(
        &'a self,
        input: DiscoverNetworkVolumesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<NetworkVolumeObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_provisioning_pods<'a>(
        &'a self,
        input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_provisioning_pod<'a>(
        &'a self,
        input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>>;

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_endpoint_templates<'a>(
        &'a self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<EndpointTemplateObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn discover_serverless_endpoints<'a>(
        &'a self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ServerlessEndpointObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn get_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>>;
}

impl<S> RunPodResourceGateway for RunPodResourceClient<S>
where
    S: SecretStore,
{
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn get_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .get_network_volume(&api_key, volume_id)
                .await
                .map(runpod_network_volume_observation)
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn discover_network_volumes<'a>(
        &'a self,
        input: DiscoverNetworkVolumesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<NetworkVolumeObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .delete_network_volume(&api_key, volume_id)
                .await
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn discover_provisioning_pods<'a>(
        &'a self,
        input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn get_provisioning_pod<'a>(
        &'a self,
        input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
                .get_pod(&api_key, &input.provider_resource_id)
                .await
                .map(runpod_pod_observation)
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .delete_pod(&api_key, pod_id)
                .await
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn discover_endpoint_templates<'a>(
        &'a self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<EndpointTemplateObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn get_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .get_template(&api_key, template_id)
                .await
                .map(runpod_template_observation)
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .delete_template(&api_key, template_id)
                .await
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn discover_serverless_endpoints<'a>(
        &'a self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ServerlessEndpointObservation>, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            self.runpod
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
        })
    }

    fn get_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .get_endpoint(&api_key, endpoint_id)
                .await
                .map(runpod_endpoint_observation)
                .map_err(WorkspaceResourceError::from)
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            self.runpod
                .delete_endpoint(&api_key, endpoint_id)
                .await
                .map_err(WorkspaceResourceError::from)
        })
    }
}

impl<S> RunPodResourceClient<S>
where
    S: SecretStore,
{
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
