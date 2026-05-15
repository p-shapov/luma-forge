use serde::{Deserialize, Serialize};

use super::{placement::PlacementPlan, provider_setup::GpuCloudProviderId};

pub mod validator;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLifecycleState {
    Draft,
    Provisioning,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResourceStatus {
    Creating,
    Running,
    Ready,
    Terminated,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentStorageVolumeSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioned_size_bytes: u64,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningPodSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub provisioner_status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerlessEndpointSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub endpoint_invoke_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum ProviderProvisioningSnapshot {
    Runpod {
        endpoint_template_snapshot: Option<RunPodEndpointTemplateSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodEndpointTemplateSnapshot {
    pub template_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub endpoint_worker_image_ref: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceProvisioningStatus {
    Idle,
    Running,
    Cancelling,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceProvisioningPhase {
    NotStarted,
    CreatingVolume,
    StartingProvisioningPod,
    PreparingEnvironment,
    CreatingEndpointTemplate,
    CreatingEndpoint,
    ValidatingReadiness,
    CleaningUp,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceProvisioningProgress {
    pub status: WorkspaceProvisioningStatus,
    pub phase: WorkspaceProvisioningPhase,
    pub percent: Option<u8>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub lifecycle_state: WorkspaceLifecycleState,
    pub placement_plan: PlacementPlan,
    pub persistent_storage_volume_snapshot: Option<PersistentStorageVolumeSnapshot>,
    pub active_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    pub serverless_endpoint_snapshot: Option<ServerlessEndpointSnapshot>,
    pub last_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    #[serde(default)]
    pub provider_provisioning_snapshot: Option<ProviderProvisioningSnapshot>,
    pub environment_prepared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceValidationError;

impl Workspace {
    pub fn new_draft(
        gpu_cloud_provider_id: GpuCloudProviderId,
        id: String,
        name: String,
        placement_plan: PlacementPlan,
    ) -> Result<Self, WorkspaceValidationError> {
        if id.trim().is_empty()
            || name.trim().is_empty()
            || placement_plan.gpu_cloud_provider_id() != gpu_cloud_provider_id
        {
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
            provider_provisioning_snapshot: None,
            environment_prepared_at: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}

#[cfg(test)]
mod tests;
