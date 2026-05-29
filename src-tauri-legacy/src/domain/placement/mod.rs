use serde::{Deserialize, Serialize};

use super::{provider_setup::GpuCloudProviderId, workflow::WorkflowPreset};

pub mod validator;

pub const RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS: u32 = 5;
pub const RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS: u32 = 5;
pub const RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS: u32 = 3600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPlacementCapabilities {
    pub endpoint_keep_alive: EndpointKeepAliveCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "supported", rename_all = "snake_case")]
pub enum EndpointKeepAliveCapability {
    #[serde(rename = "true")]
    Supported {
        default_seconds: u32,
        min_seconds: u32,
        max_seconds: u32,
    },
    #[serde(rename = "false")]
    Unsupported,
}

impl ProviderPlacementCapabilities {
    pub fn for_provider(provider_id: GpuCloudProviderId) -> Self {
        match provider_id {
            GpuCloudProviderId::Runpod => Self {
                endpoint_keep_alive: EndpointKeepAliveCapability::Supported {
                    default_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
                    min_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS,
                    max_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS,
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum PlacementPlan {
    Runpod {
        selected_datacenter_id: String,
        selected_gpu_id: String,
        persistent_storage_volume_size_bytes: u64,
        #[serde(default = "default_runpod_endpoint_keep_alive_seconds")]
        endpoint_keep_alive_seconds: u32,
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

fn default_runpod_endpoint_keep_alive_seconds() -> u32 {
    RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS
}
