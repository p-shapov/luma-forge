use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        provider_inventory::{
            Datacenter as DomainDatacenter, GpuOption as DomainGpuOption,
            ProviderInventory as DomainProviderInventory,
        },
        provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId,
    },
    shared_contracts::provider_contracts::GpuCloudProviderId,
    workspace::workspace_contracts::{
        EndpointProfile, PlacementPlan, ProvisioningProfile, WorkflowCatalog, Workspace,
        WorkspaceCatalog,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: WorkflowCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<ProvisioningProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<EndpointProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProviderInventoryResponse {
    pub provider_inventory: ProviderInventory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuOption {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

impl From<DomainGpuOption> for GpuOption {
    fn from(option: DomainGpuOption) -> Self {
        Self {
            gpu_cloud_provider_id: option.gpu_cloud_provider_id.into(),
            id: option.id,
            name: option.name,
            vram_bytes: option.vram_bytes,
            availability_score: option.availability_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Datacenter {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<GpuOption>,
}

impl From<DomainDatacenter> for Datacenter {
    fn from(datacenter: DomainDatacenter) -> Self {
        Self {
            gpu_cloud_provider_id: datacenter.gpu_cloud_provider_id.into(),
            id: datacenter.id,
            name: datacenter.name,
            gpu_options: datacenter.gpu_options.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInventory {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub fetched_at: String,
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<Datacenter>,
}

impl From<DomainProviderInventory> for ProviderInventory {
    fn from(inventory: DomainProviderInventory) -> Self {
        Self {
            gpu_cloud_provider_id: inventory.gpu_cloud_provider_id.into(),
            fetched_at: inventory.fetched_at,
            max_persistent_storage_volume_size_bytes: inventory
                .max_persistent_storage_volume_size_bytes,
            datacenters: inventory.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: WorkspaceCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}

impl CreateWorkspaceRequest {
    pub fn domain_provider_id(&self) -> DomainGpuCloudProviderId {
        self.gpu_cloud_provider_id.into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceResponse {
    pub workspace: Workspace,
}
