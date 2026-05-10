use super::provider_setup::GpuCloudProviderId;
use serde::{Deserialize, Serialize};

pub mod validator;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuOption {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Datacenter {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<GpuOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInventory {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub fetched_at: String,
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<Datacenter>,
}
