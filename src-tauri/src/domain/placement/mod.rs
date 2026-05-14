use serde::{Deserialize, Serialize};

use super::{provider_setup::GpuCloudProviderId, workflow::WorkflowPreset};

pub mod validator;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum PlacementPlan {
    Runpod {
        selected_datacenter_id: String,
        selected_gpu_id: String,
        persistent_storage_volume_size_bytes: u64,
        selected_workflow_preset: WorkflowPreset,
    },
}

impl PlacementPlan {
    pub fn gpu_cloud_provider_id(&self) -> GpuCloudProviderId {
        match self {
            Self::Runpod { .. } => GpuCloudProviderId::Runpod,
        }
    }

    pub fn selected_workflow_preset(&self) -> &WorkflowPreset {
        match self {
            Self::Runpod {
                selected_workflow_preset,
                ..
            } => selected_workflow_preset,
        }
    }
}
