use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    bundled::bundled_catalog_contracts::{
        EndpointProfile, ProvisioningProfile, RunPodEndpointProfileConfig,
        RunPodProvisioningProfileConfig,
    },
    domain::{
        placement::PlacementPlan as DomainPlacementPlan,
        provider_setup::GpuCloudProviderId,
        workflow::WorkflowPreset,
        workspace::{
            PersistentStorageVolumeSnapshot, ProvisioningPodSnapshot, ServerlessEndpointSnapshot,
            WorkspaceLifecycleState,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PlacementPlan {
    pub selected_datacenter_id: String,
    pub selected_gpu_id: String,
    pub persistent_storage_volume_size_bytes: u64,
    pub selected_workflow_preset: WorkflowPreset,
    pub selected_provisioning_profile: ProvisioningProfile,
    pub selected_endpoint_profile: EndpointProfile,
}

impl PlacementPlan {
    pub fn to_domain(
        &self,
    ) -> DomainPlacementPlan<
        crate::domain::profiles::ProvisioningProfile<RunPodProvisioningProfileConfig>,
        crate::domain::profiles::EndpointProfile<RunPodEndpointProfileConfig>,
    > {
        DomainPlacementPlan {
            selected_datacenter_id: self.selected_datacenter_id.clone(),
            selected_gpu_id: self.selected_gpu_id.clone(),
            persistent_storage_volume_size_bytes: self.persistent_storage_volume_size_bytes,
            selected_workflow_preset: self.selected_workflow_preset.clone(),
            selected_provisioning_profile: self.selected_provisioning_profile.to_domain(),
            selected_endpoint_profile: self.selected_endpoint_profile.to_domain(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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
    pub environment_prepared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}
