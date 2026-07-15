use serde::{Deserialize, Serialize};

use crate::application::runtimes::runpod::{
    RunpodPlacement, RunpodPlacementDatacenter, RunpodPlacementGpu,
};

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementDto {
    pub max_volume_size_gb: u64,
    pub datacenters: Vec<RunpodPlacementDatacenterDto>,
}

impl From<RunpodPlacement> for RunpodPlacementDto {
    fn from(value: RunpodPlacement) -> Self {
        Self {
            max_volume_size_gb: value.max_volume_size_gb,
            datacenters: value.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementDatacenterDto {
    pub id: String,
    pub name: String,
    pub gpus: Vec<RunpodPlacementGpuDto>,
}

impl From<RunpodPlacementDatacenter> for RunpodPlacementDatacenterDto {
    fn from(value: RunpodPlacementDatacenter) -> Self {
        Self {
            id: value.id,
            name: value.name,
            gpus: value.gpus.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementGpuDto {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
}

impl From<RunpodPlacementGpu> for RunpodPlacementGpuDto {
    fn from(value: RunpodPlacementGpu) -> Self {
        Self {
            id: value.id,
            name: value.name,
            vram_gb: value.vram_gb,
        }
    }
}
