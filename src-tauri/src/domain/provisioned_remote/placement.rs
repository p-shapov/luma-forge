use serde::{Deserialize, Serialize};

use super::provider::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteGpuPlacementOption {
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDatacenterPlacementOption {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<RemoteGpuPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePlacementOptions {
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<RemoteDatacenterPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpointKeepAliveLimits {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePlacementPlan {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_bytes: u64,
    pub keep_alive_limits: Option<RemoteEndpointKeepAliveLimits>,
}
