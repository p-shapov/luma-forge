use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::runpod::{
    RunpodDatacenterPlacementOption, RunpodGpuPlacementOption, RunpodPlacementOptions,
    RunpodPlacementPlan,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodGpuPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodDatacenterPlacementOptionResponse {
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<RunpodGpuPlacementOptionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementOptionsResponse {
    pub max_network_volume_size_gb: Option<u64>,
    pub datacenters: Vec<RunpodDatacenterPlacementOptionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodEndpointKeepAliveLimitsDto {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementPlanInput {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimitsDto>,
}

impl From<RunpodPlacementPlanInput> for RunpodPlacementPlan {
    fn from(value: RunpodPlacementPlanInput) -> Self {
        Self {
            data_center_id: value.datacenter_id,
            gpu_type_id: value.gpu_id,
            volume_size_gb: value.volume_size_gb,
        }
    }
}

impl From<RunpodPlacementPlan> for RunpodPlacementPlanInput {
    fn from(value: RunpodPlacementPlan) -> Self {
        Self {
            datacenter_id: value.data_center_id,
            gpu_id: value.gpu_type_id,
            volume_size_gb: value.volume_size_gb,
            keep_alive_limits: None,
        }
    }
}

impl From<RunpodGpuPlacementOption> for RunpodGpuPlacementOptionResponse {
    fn from(value: RunpodGpuPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            vram_gb: value.vram_gb,
        }
    }
}

impl From<RunpodDatacenterPlacementOption> for RunpodDatacenterPlacementOptionResponse {
    fn from(value: RunpodDatacenterPlacementOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            gpu_options: value.gpu_options.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RunpodPlacementOptions> for RunpodPlacementOptionsResponse {
    fn from(value: RunpodPlacementOptions) -> Self {
        Self {
            max_network_volume_size_gb: value.max_volume_size_gb,
            datacenters: value.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_plan_input_uses_gb_domain_value() {
        let plan = RunpodPlacementPlan::from(RunpodPlacementPlanInput {
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_gb: 19,
            keep_alive_limits: None,
        });

        assert_eq!(plan.volume_size_gb, 19);
    }

    #[test]
    fn placement_plan_input_preserves_zero_volume_for_later_validation() {
        let plan = RunpodPlacementPlan::from(RunpodPlacementPlanInput {
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_gb: 0,
            keep_alive_limits: None,
        });

        assert_eq!(plan.volume_size_gb, 0);
    }

    #[test]
    fn placement_plan_response_uses_gb_volume_field() {
        let input = RunpodPlacementPlanInput::from(RunpodPlacementPlan {
            data_center_id: "dc".to_string(),
            gpu_type_id: "gpu".to_string(),
            volume_size_gb: 19,
        });

        assert_eq!(input.volume_size_gb, 19);
    }

    #[test]
    fn placement_options_response_uses_gb_fields() {
        let response = RunpodPlacementOptionsResponse::from(RunpodPlacementOptions {
            max_volume_size_gb: Some(4_000),
            datacenters: vec![RunpodDatacenterPlacementOption {
                id: "dc".to_string(),
                name: "Datacenter".to_string(),
                gpu_options: vec![RunpodGpuPlacementOption {
                    id: "gpu".to_string(),
                    name: "GPU".to_string(),
                    vram_gb: 24,
                }],
            }],
        });

        assert_eq!(response.max_network_volume_size_gb, Some(4_000));
        assert_eq!(response.datacenters[0].gpu_options[0].vram_gb, 24);
    }
}
