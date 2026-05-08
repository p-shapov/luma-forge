use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    bundled::bundled_catalog_contracts::{EndpointProfile, ProvisioningProfile},
    domain::{
        provider_inventory::ProviderInventory, provider_setup::GpuCloudProviderId,
        workflow::WorkflowCatalog,
    },
    workspace::workspace_contracts::{PlacementPlan, Workspace, WorkspaceCatalog},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: WorkflowCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<ProvisioningProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<EndpointProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryResponse {
    pub provider_inventory: ProviderInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: WorkspaceCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceResponse {
    pub workspace: Workspace,
}
