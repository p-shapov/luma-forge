use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provisioned_remote::{
    RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits, RemoteGpuPlacementOption,
    RemotePlacementOptions, RemotePlacementPlan,
};

use super::provider::GpuCloudProviderIdDto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGpuPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDatacenterPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<RemoteGpuPlacementOptionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlacementOptionsResponse {
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<RemoteDatacenterPlacementOptionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEndpointKeepAliveLimitsDto {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlacementPlanInput {
    pub gpu_cloud_provider_id: GpuCloudProviderIdDto,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_bytes: u64,
    pub keep_alive_limits: Option<RemoteEndpointKeepAliveLimitsDto>,
}

impl From<RemoteEndpointKeepAliveLimitsDto> for RemoteEndpointKeepAliveLimits {
    fn from(value: RemoteEndpointKeepAliveLimitsDto) -> Self {
        Self {
            default_seconds: value.default_seconds,
            min_seconds: value.min_seconds,
            max_seconds: value.max_seconds,
        }
    }
}

impl From<RemoteEndpointKeepAliveLimits> for RemoteEndpointKeepAliveLimitsDto {
    fn from(value: RemoteEndpointKeepAliveLimits) -> Self {
        Self {
            default_seconds: value.default_seconds,
            min_seconds: value.min_seconds,
            max_seconds: value.max_seconds,
        }
    }
}

impl From<RemotePlacementPlanInput> for RemotePlacementPlan {
    fn from(value: RemotePlacementPlanInput) -> Self {
        Self {
            gpu_cloud_provider_id: value.gpu_cloud_provider_id.into(),
            datacenter_id: value.datacenter_id,
            gpu_id: value.gpu_id,
            volume_size_bytes: value.volume_size_bytes,
            keep_alive_limits: value.keep_alive_limits.map(Into::into),
        }
    }
}

impl From<RemotePlacementPlan> for RemotePlacementPlanInput {
    fn from(value: RemotePlacementPlan) -> Self {
        Self {
            gpu_cloud_provider_id: value.gpu_cloud_provider_id.into(),
            datacenter_id: value.datacenter_id,
            gpu_id: value.gpu_id,
            volume_size_bytes: value.volume_size_bytes,
            keep_alive_limits: value.keep_alive_limits.map(Into::into),
        }
    }
}

impl From<RemoteGpuPlacementOption> for RemoteGpuPlacementOptionResponse {
    fn from(value: RemoteGpuPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            vram_bytes: value.vram_bytes,
            availability_score: value.availability_score,
        }
    }
}

impl From<RemoteDatacenterPlacementOption> for RemoteDatacenterPlacementOptionResponse {
    fn from(value: RemoteDatacenterPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            gpu_options: value.gpu_options.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RemotePlacementOptions> for RemotePlacementOptionsResponse {
    fn from(value: RemotePlacementOptions) -> Self {
        Self {
            max_persistent_storage_volume_size_bytes: value
                .max_persistent_storage_volume_size_bytes,
            datacenters: value.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}
