use std::collections::HashMap;

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::Workspace,
    },
    provider::runpod::{
        RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest, RunPodCreatePodRequest,
        RunPodCreateTemplateRequest, RunPodEndpointObservation, RunPodNetworkVolumeObservation,
        RunPodPodObservation, RunPodTemplateObservation,
    },
    secrets::AsyncSecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::contracts::{
        CreateEndpointTemplateInput, DiscoverEndpointTemplatesInput, EndpointTemplateObservation,
    },
    workspace_resources::{
        provider_resource_name, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, DiscoverNetworkVolumesInput, DiscoverProvisioningPodsInput,
        DiscoverServerlessEndpointsInput, NetworkVolumeObservation, ObserveProvisioningPodInput,
        ProvisioningPodObservation, ServerlessEndpointObservation, WorkspaceResourceContext,
        WorkspaceResourceError,
    },
};

use super::{
    client::RunPodWorkspaceResourceClient, GIB_BYTES, RUNPOD_ENDPOINT_COMFYUI_HTTP_PORT,
    RUNPOD_PROVISIONER_WORKER_HTTP_PORT,
};

const RUNPOD_PROVISIONING_POD_COMPUTE_TYPE: &str = "CPU";
const RUNPOD_PROVISIONING_POD_CPU_FLAVOR_ID: &str = "cpu3g";
const RUNPOD_PROVISIONING_POD_CPU_FLAVOR_PRIORITY: &str = "availability";
const RUNPOD_PROVISIONING_POD_VCPU_COUNT: u32 = 2;

pub(super) struct RunPodWorkspaceResourceContext<'a, S, W, C> {
    base: &'a WorkspaceResourceContext<'a, S, W>,
    client: &'a C,
    pub(crate) secrets: &'a S,
}

impl<'a, S, W, C> RunPodWorkspaceResourceContext<'a, S, W, C> {
    pub(super) fn new(base: &'a WorkspaceResourceContext<'a, S, W>, client: &'a C) -> Self {
        Self {
            base,
            client,
            secrets: base.secrets,
        }
    }
}

impl<S, W, C> RunPodWorkspaceResourceContext<'_, S, W, C>
where
    W: WorkspaceCatalogRepository,
{
    pub(super) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        self.base.update_workspace(workspace).await
    }
}

impl<S, W, C> RunPodWorkspaceResourceContext<'_, S, W, C>
where
    S: AsyncSecretStore,
    C: RunPodWorkspaceResourceClient,
{
    pub(super) async fn create_network_volume(
        &self,
        input: CreateNetworkVolumeInput,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn get_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<NetworkVolumeObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .get_network_volume(&api_key, volume_id)
            .await
            .map(runpod_network_volume_observation)
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn discover_network_volumes(
        &self,
        input: DiscoverNetworkVolumesInput,
    ) -> Result<Vec<NetworkVolumeObservation>, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn delete_network_volume(
        &self,
        provider_id: GpuCloudProviderId,
        volume_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .delete_network_volume(&api_key, volume_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn create_provisioning_pod(
        &self,
        input: CreateProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
            .create_pod(
                &api_key,
                &RunPodCreatePodRequest {
                    name: provider_resource_name(&input.workspace_id, "provisioner"),
                    image_name: input.provisioner_worker_image_ref,
                    compute_type: RUNPOD_PROVISIONING_POD_COMPUTE_TYPE.to_string(),
                    cpu_flavor_ids: vec![RUNPOD_PROVISIONING_POD_CPU_FLAVOR_ID.to_string()],
                    cpu_flavor_priority: RUNPOD_PROVISIONING_POD_CPU_FLAVOR_PRIORITY.to_string(),
                    vcpu_count: RUNPOD_PROVISIONING_POD_VCPU_COUNT,
                    gpu_type_ids: None,
                    data_center_ids: vec![input.datacenter_id],
                    network_volume_id: input.network_volume_id,
                    volume_mount_path: input.mount_path.clone(),
                    env: HashMap::from([
                        (
                            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
                            input.bearer_token.expose_secret().to_string(),
                        ),
                        (
                            "LUMA_FORGE_WORKSPACE_MOUNT_PATH".to_string(),
                            input.mount_path,
                        ),
                    ]),
                    ports: vec![format!("{RUNPOD_PROVISIONER_WORKER_HTTP_PORT}/http")],
                },
            )
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn discover_provisioning_pods(
        &self,
        input: DiscoverProvisioningPodsInput,
    ) -> Result<Vec<ProvisioningPodObservation>, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn get_provisioning_pod(
        &self,
        input: ObserveProvisioningPodInput,
    ) -> Result<ProvisioningPodObservation, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
            .get_pod(&api_key, &input.provider_resource_id)
            .await
            .map(runpod_pod_observation)
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn delete_provisioning_pod(
        &self,
        provider_id: GpuCloudProviderId,
        pod_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .delete_pod(&api_key, pod_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn create_endpoint_template(
        &self,
        input: CreateEndpointTemplateInput,
    ) -> Result<EndpointTemplateObservation, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
            .create_template(
                &api_key,
                &RunPodCreateTemplateRequest {
                    name: provider_resource_name(&input.workspace_id, "endpoint-template"),
                    image_name: input.endpoint_worker_image_ref,
                    container_disk_in_gb: 10,
                    env: HashMap::from([(
                        "LUMA_FORGE_WORKSPACE_MOUNT_PATH".to_string(),
                        input.mount_path.clone(),
                    )]),
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

    pub(super) async fn discover_endpoint_templates(
        &self,
        input: DiscoverEndpointTemplatesInput,
    ) -> Result<Vec<EndpointTemplateObservation>, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn delete_endpoint_template(
        &self,
        provider_id: GpuCloudProviderId,
        template_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .delete_template(&api_key, template_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn create_serverless_endpoint(
        &self,
        input: CreateServerlessEndpointInput,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn discover_serverless_endpoints(
        &self,
        input: DiscoverServerlessEndpointsInput,
    ) -> Result<Vec<ServerlessEndpointObservation>, WorkspaceResourceError> {
        let api_key = self
            .provisioning_api_key(&input.gpu_cloud_provider_id)
            .await?;
        self.client
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

    pub(super) async fn get_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<ServerlessEndpointObservation, WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .get_endpoint(&api_key, endpoint_id)
            .await
            .map(runpod_endpoint_observation)
            .map_err(WorkspaceResourceError::from)
    }

    pub(super) async fn delete_serverless_endpoint(
        &self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &str,
    ) -> Result<(), WorkspaceResourceError> {
        let api_key = self.provisioning_api_key(&provider_id).await?;
        self.client
            .delete_endpoint(&api_key, endpoint_id)
            .await
            .map_err(WorkspaceResourceError::from)
    }

    async fn provisioning_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<ProviderApiKey, WorkspaceResourceError> {
        self.secrets
            .read_api_key(provider_id)
            .await
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
