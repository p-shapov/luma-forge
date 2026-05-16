use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::{ProviderResourceStatus, Workspace, WorkspaceProvisioningProgress},
    },
    secrets::ProvisionerWorkerBearerToken,
};

#[derive(Debug, Clone)]
pub struct CreateNetworkVolumeInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub datacenter_id: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct NetworkVolumeObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provisioned_size_bytes: u64,
    pub provider_resource_status: ProviderResourceStatus,
    pub mount_path: String,
}

#[derive(Debug, Clone)]
pub struct DiscoverNetworkVolumesInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub datacenter_id: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CreateProvisioningPodInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub provisioner_worker_image_ref: String,
    pub provisioner_worker_port: u16,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub bearer_token: ProvisionerWorkerBearerToken,
}

#[derive(Debug, Clone)]
pub struct DiscoverProvisioningPodsInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub expected_provisioner_worker_image_ref: String,
}

#[derive(Debug, Clone)]
pub struct ObserveProvisioningPodInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub expected_provisioner_worker_image_ref: String,
}

#[derive(Debug, Clone)]
pub struct ProvisioningPodObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub provisioner_worker_image_ref: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioner_status_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateEndpointTemplateInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub endpoint_worker_image_ref: String,
    pub endpoint_worker_port: u16,
    pub mount_path: String,
}

#[derive(Debug, Clone)]
pub struct EndpointTemplateObservation {
    pub template_id: String,
    pub endpoint_worker_image_ref: String,
    pub mount_path: String,
    pub provider_resource_status: ProviderResourceStatus,
}

#[derive(Debug, Clone)]
pub struct DiscoverEndpointTemplatesInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub endpoint_worker_image_ref: String,
    pub endpoint_worker_port: u16,
    pub mount_path: String,
}

#[derive(Debug, Clone)]
pub struct CreateServerlessEndpointInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub template_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub endpoint_keep_alive_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct ServerlessEndpointObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub endpoint_invoke_url: String,
}

#[derive(Debug, Clone)]
pub struct DiscoverServerlessEndpointsInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub template_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub endpoint_keep_alive_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct WorkspaceProvisioningResult {
    pub workspace: Workspace,
    pub progress: WorkspaceProvisioningProgress,
}
