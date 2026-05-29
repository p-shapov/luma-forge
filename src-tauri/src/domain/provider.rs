use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuCloudProviderId {
    Runpod,
}

impl GpuCloudProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderGpuOption {
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDatacenter {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<ProviderGpuOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInventory {
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<ProviderDatacenter>,
}
