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
pub struct RunpodPlacementPlanInput {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
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
