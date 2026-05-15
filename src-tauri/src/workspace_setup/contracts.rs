use crate::domain::{
    placement::{PlacementPlan, ProviderPlacementCapabilities},
    provider_inventory::ProviderInventory,
    provider_setup::GpuCloudProviderId,
};

#[derive(Debug, Clone)]
pub struct CreateWorkspaceInput {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlacementOptions {
    pub provider_inventory: ProviderInventory,
    pub placement_capabilities: ProviderPlacementCapabilities,
}
