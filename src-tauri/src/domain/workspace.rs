use super::{placement::PlacementPlan, provider_setup::GpuCloudProviderId};

#[derive(Debug, Clone, PartialEq, Eq)]
// Workspace Provisioning will construct the non-Draft variants when lifecycle
// transitions land; keep the domain vocabulary aligned with the flow specs.
#[allow(dead_code)]
pub enum WorkspaceLifecycleState {
    Draft,
    Provisioning,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// Workspace Provisioning will construct these statuses when Provider Resource
// snapshots are updated from provider observations.
#[allow(dead_code)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace<ProvisioningProfile, EndpointProfile> {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub lifecycle_state: WorkspaceLifecycleState,
    pub placement_plan: PlacementPlan<ProvisioningProfile, EndpointProfile>,
    pub persistent_storage_volume_snapshot: Option<PersistentStorageVolumeSnapshot>,
    pub active_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    pub serverless_endpoint_snapshot: Option<ServerlessEndpointSnapshot>,
    pub last_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    pub environment_prepared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceValidationError;

impl<ProvisioningProfile, EndpointProfile> Workspace<ProvisioningProfile, EndpointProfile> {
    pub fn new_draft(
        gpu_cloud_provider_id: GpuCloudProviderId,
        id: String,
        name: String,
        placement_plan: PlacementPlan<ProvisioningProfile, EndpointProfile>,
    ) -> Result<Self, WorkspaceValidationError> {
        if id.trim().is_empty() || name.trim().is_empty() {
            return Err(WorkspaceValidationError);
        }

        Ok(Self {
            gpu_cloud_provider_id,
            id,
            name,
            lifecycle_state: WorkspaceLifecycleState::Draft,
            placement_plan,
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            environment_prepared_at: None,
        })
    }
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod workspace_tests;
