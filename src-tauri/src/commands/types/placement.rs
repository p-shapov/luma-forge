use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::provisioned_remote::{
    RunpodDatacenterPlacementOption, RunpodEndpointKeepAliveLimits, RunpodGpuPlacementOption,
    RunpodPlacementOptions, RunpodPlacementPlan,
};

use super::provider::GpuCloudProviderIdDto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGpuPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
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
    pub max_network_volume_size_gb: Option<u64>,
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
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RemoteEndpointKeepAliveLimitsDto>,
}

impl From<RemoteEndpointKeepAliveLimitsDto> for RunpodEndpointKeepAliveLimits {
    fn from(value: RemoteEndpointKeepAliveLimitsDto) -> Self {
        Self {
            default_seconds: value.default_seconds,
            min_seconds: value.min_seconds,
            max_seconds: value.max_seconds,
        }
    }
}

impl From<RunpodEndpointKeepAliveLimits> for RemoteEndpointKeepAliveLimitsDto {
    fn from(value: RunpodEndpointKeepAliveLimits) -> Self {
        Self {
            default_seconds: value.default_seconds,
            min_seconds: value.min_seconds,
            max_seconds: value.max_seconds,
        }
    }
}

impl From<RemotePlacementPlanInput> for RunpodPlacementPlan {
    fn from(value: RemotePlacementPlanInput) -> Self {
        Self {
            data_center_id: value.datacenter_id,
            gpu_type_id: value.gpu_id,
            volume_size_gb: value.volume_size_gb,
            keep_alive_limits: value.keep_alive_limits.map(Into::into),
        }
    }
}

impl From<RunpodPlacementPlan> for RemotePlacementPlanInput {
    fn from(value: RunpodPlacementPlan) -> Self {
        Self {
            gpu_cloud_provider_id: GpuCloudProviderIdDto::Runpod,
            datacenter_id: value.data_center_id,
            gpu_id: value.gpu_type_id,
            volume_size_gb: value.volume_size_gb,
            keep_alive_limits: value.keep_alive_limits.map(Into::into),
        }
    }
}

impl From<RunpodGpuPlacementOption> for RemoteGpuPlacementOptionResponse {
    fn from(value: RunpodGpuPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            vram_gb: value.vram_gb,
            availability_score: value.availability_score,
        }
    }
}

impl From<RunpodDatacenterPlacementOption> for RemoteDatacenterPlacementOptionResponse {
    fn from(value: RunpodDatacenterPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            gpu_options: value.gpu_options.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RunpodPlacementOptions> for RemotePlacementOptionsResponse {
    fn from(value: RunpodPlacementOptions) -> Self {
        Self {
            max_network_volume_size_gb: value.max_network_volume_size_gb,
            datacenters: value.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_plan_input_uses_gb_domain_value() {
        let plan = RunpodPlacementPlan::from(RemotePlacementPlanInput {
            gpu_cloud_provider_id: GpuCloudProviderIdDto::Runpod,
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_gb: 19,
            keep_alive_limits: None,
        });

        assert_eq!(plan.volume_size_gb, 19);
    }

    #[test]
    fn placement_plan_input_preserves_zero_volume_for_later_validation() {
        let plan = RunpodPlacementPlan::from(RemotePlacementPlanInput {
            gpu_cloud_provider_id: GpuCloudProviderIdDto::Runpod,
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_gb: 0,
            keep_alive_limits: None,
        });

        assert_eq!(plan.volume_size_gb, 0);
    }

    #[test]
    fn placement_plan_response_uses_gb_volume_field() {
        let input = RemotePlacementPlanInput::from(RunpodPlacementPlan {
            data_center_id: "dc".to_string(),
            gpu_type_id: "gpu".to_string(),
            volume_size_gb: 19,
            keep_alive_limits: None,
        });

        assert_eq!(input.volume_size_gb, 19);
    }

    #[test]
    fn placement_options_response_uses_gb_fields() {
        let response = RemotePlacementOptionsResponse::from(RunpodPlacementOptions {
            max_network_volume_size_gb: Some(4_000),
            datacenters: vec![RunpodDatacenterPlacementOption {
                id: "dc".to_string(),
                name: "Datacenter".to_string(),
                gpu_options: vec![RunpodGpuPlacementOption {
                    id: "gpu".to_string(),
                    name: "GPU".to_string(),
                    vram_gb: 24,
                    availability_score: 100,
                }],
            }],
        });

        assert_eq!(response.max_network_volume_size_gb, Some(4_000));
        assert_eq!(response.datacenters[0].gpu_options[0].vram_gb, 24);
    }
}
