use serde::{Deserialize, Serialize};

use super::provider::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePlacementCapabilities {
    pub remote_endpoint_keep_alive: RemoteEndpointKeepAliveCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "supported", rename_all = "snake_case")]
pub enum Capability<T> {
    #[serde(rename = "true")]
    Supported(T),

    #[serde(rename = "false")]
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpointKeepAliveLimits {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

pub type RemoteEndpointKeepAliveCapability = Capability<RemoteEndpointKeepAliveLimits>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePlacementPlan {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub remote_volume_size_bytes: u64,
    pub remote_capabilities: RemotePlacementCapabilities,
}
