#![allow(dead_code)]

use super::provider_setup::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceLifecycleState {
    Draft,
    Provisioning,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderResourceStatus {
    Creating,
    Running,
    Ready,
    Terminated,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentStorageVolumeSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioned_size_bytes: u64,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningPodSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub provisioning_profile_id: String,
    pub provisioner_status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerlessEndpointSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub endpoint_profile_id: String,
    pub endpoint_invoke_url: String,
}
