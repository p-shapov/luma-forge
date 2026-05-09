use super::workflow::WorkflowPreset;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPlan<ProvisioningProfile, EndpointProfile> {
    pub selected_datacenter_id: String,
    pub selected_gpu_id: String,
    pub persistent_storage_volume_size_bytes: u64,
    pub selected_workflow_preset: WorkflowPreset,
    pub selected_provisioning_profile: ProvisioningProfile,
    pub selected_endpoint_profile: EndpointProfile,
}
