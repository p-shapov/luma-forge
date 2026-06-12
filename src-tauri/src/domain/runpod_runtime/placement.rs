use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodGpuPlacementOption {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodDatacenterPlacementOption {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<RunpodGpuPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodPlacementOptions {
    pub max_network_volume_size_gb: Option<u64>,
    pub datacenters: Vec<RunpodDatacenterPlacementOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodEndpointKeepAliveLimits {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunpodPlacementPlan {
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}
