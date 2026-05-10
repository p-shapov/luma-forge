use crate::domain::{placement::PlacementPlan, provider_setup::GpuCloudProviderId};

#[derive(Debug, Clone)]
pub struct CreateWorkspaceInput {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}
