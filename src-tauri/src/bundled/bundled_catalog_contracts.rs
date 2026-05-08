use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{
    profiles::{
        EndpointProfile as DomainEndpointProfile, EndpointWorkerRuntime, ProvisionerWorkerRuntime,
        ProvisioningProfile as DomainProvisioningProfile,
    },
    provider_setup::GpuCloudProviderId,
    shared::EnvironmentVariables,
    workflow::WorkflowExecutionType,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodProvisioningProfileConfig {
    pub cloud_type: Option<String>,
    pub pod_template_id: Option<String>,
    pub network_volume_mount_path: String,
    pub expose_http_ports: Vec<u16>,
    pub env: Option<EnvironmentVariables>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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
    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }

    pub fn to_domain(&self) -> DomainProvisioningProfile<RunPodProvisioningProfileConfig> {
        match self {
            Self::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
            } => DomainProvisioningProfile {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                id: id.clone(),
                version: version.clone(),
                name: name.clone(),
                provisioner_worker_runtime: provisioner_worker_runtime.clone(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodServerlessScalingConfig {
    pub min_workers: u32,
    pub max_workers: u32,
    pub idle_timeout_seconds: u32,
    pub scaler_type: Option<String>,
    pub scaler_value: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodEndpointProfileConfig {
    pub endpoint_template_id: Option<String>,
    pub container_disk_bytes: u64,
    pub volume_mount_path: String,
    pub env: Option<EnvironmentVariables>,
    pub scaling: RunPodServerlessScalingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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
    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }

    pub fn to_domain(&self) -> DomainEndpointProfile<RunPodEndpointProfileConfig> {
        match self {
            Self::Runpod {
                id,
                version,
                name,
                workflow_execution_type,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
            } => DomainEndpointProfile {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                id: id.clone(),
                version: version.clone(),
                name: name.clone(),
                workflow_execution_type: workflow_execution_type.clone(),
                endpoint_worker_runtime: endpoint_worker_runtime.clone(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.clone(),
            },
        }
    }
}
