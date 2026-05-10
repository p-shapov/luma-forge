use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{provider_setup::GpuCloudProviderId, workflow::WorkflowExecutionType};

pub mod validator;

pub type EnvironmentVariables = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum ProvisioningProfile {
    Runpod {
        id: String,
        version: String,
        name: String,
        provisioner_worker_runtime: ProvisionerWorkerRuntime,
        gpu_cloud_provider_config: RunPodProvisioningProfileConfig,
    },
}

impl ProvisioningProfile {
    pub fn gpu_cloud_provider_id(&self) -> GpuCloudProviderId {
        match self {
            Self::Runpod { .. } => GpuCloudProviderId::Runpod,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum EndpointProfile {
    Runpod {
        id: String,
        version: String,
        name: String,
        workflow_execution_type: WorkflowExecutionType,
        endpoint_worker_runtime: EndpointWorkerRuntime,
        gpu_cloud_provider_config: RunPodEndpointProfileConfig,
    },
}

impl EndpointProfile {
    pub fn gpu_cloud_provider_id(&self) -> GpuCloudProviderId {
        match self {
            Self::Runpod { .. } => GpuCloudProviderId::Runpod,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }

    pub fn workflow_execution_type(&self) -> WorkflowExecutionType {
        match self {
            Self::Runpod {
                workflow_execution_type,
                ..
            } => *workflow_execution_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodProvisioningProfileConfig {
    pub cloud_type: Option<String>,
    pub pod_template_id: Option<String>,
    pub network_volume_mount_path: String,
    pub expose_http_ports: Vec<u16>,
    pub env: Option<EnvironmentVariables>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodServerlessScalingConfig {
    pub min_workers: u32,
    pub max_workers: u32,
    pub idle_timeout_seconds: u32,
    pub scaler_type: Option<String>,
    pub scaler_value: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodEndpointProfileConfig {
    pub endpoint_template_id: Option<String>,
    pub container_disk_bytes: u64,
    pub volume_mount_path: String,
    pub env: Option<EnvironmentVariables>,
    pub scaling: RunPodServerlessScalingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningComputeType {
    Pod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningStatusEndpoint {
    pub port: u16,
    pub protocol: String,
    pub status_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerWorkerRuntime {
    pub provisioner_version: String,
    pub docker_image_ref: String,
    pub volume_mount_path: String,
    pub container_disk_bytes: u64,
    pub compute_type: ProvisioningComputeType,
    pub status_endpoint: ProvisioningStatusEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointWorkerRuntime {
    pub endpoint_worker_version: String,
    pub docker_image_ref: String,
    pub http_port: u16,
    pub health_path: String,
    pub invoke_path: String,
}
