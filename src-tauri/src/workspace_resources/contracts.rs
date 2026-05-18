use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::ProviderResourceStatus},
    secrets::ProvisionerWorkerBearerToken,
};

#[derive(Debug, Clone)]
pub(crate) struct CreateNetworkVolumeInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
    pub(crate) datacenter_id: String,
    pub(crate) size_bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkVolumeObservation {
    pub(crate) provider_resource_id: String,
    pub(crate) provider_resource_status: ProviderResourceStatus,
    pub(crate) mount_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoverNetworkVolumesInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateProvisioningPodInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
    pub(crate) provisioner_worker_image_ref: String,
    pub(crate) datacenter_id: String,
    pub(crate) selected_gpu_id: String,
    pub(crate) network_volume_id: String,
    pub(crate) mount_path: String,
    pub(crate) bearer_token: ProvisionerWorkerBearerToken,
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoverProvisioningPodsInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ObserveProvisioningPodInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) provider_resource_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ProvisioningPodObservation {
    pub(crate) provider_resource_id: String,
    pub(crate) provider_resource_status: ProviderResourceStatus,
    pub(crate) provisioner_status_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateEndpointTemplateInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
    pub(crate) endpoint_worker_image_ref: String,
    pub(crate) mount_path: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EndpointTemplateObservation {
    pub(crate) template_id: String,
    pub(crate) endpoint_worker_image_ref: String,
    pub(crate) mount_path: String,
    pub(crate) provider_resource_status: ProviderResourceStatus,
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoverEndpointTemplatesInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateServerlessEndpointInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
    pub(crate) template_id: String,
    pub(crate) datacenter_id: String,
    pub(crate) selected_gpu_id: String,
    pub(crate) network_volume_id: String,
    pub(crate) endpoint_keep_alive_seconds: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct ServerlessEndpointObservation {
    pub(crate) provider_resource_id: String,
    pub(crate) provider_resource_status: ProviderResourceStatus,
    pub(crate) endpoint_invoke_url: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoverServerlessEndpointsInput {
    pub(crate) gpu_cloud_provider_id: GpuCloudProviderId,
    pub(crate) workspace_id: String,
}
