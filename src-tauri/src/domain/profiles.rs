use super::{
    provider_setup::GpuCloudProviderId, shared::DockerImage, workflow::WorkflowExecutionType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningProfile<C> {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub version: String,
    pub name: String,
    pub provisioner_worker_runtime: ProvisionerWorkerRuntime,
    pub gpu_cloud_provider_config: C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointProfile<C> {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub version: String,
    pub name: String,
    pub workflow_execution_type: WorkflowExecutionType,
    pub endpoint_worker_runtime: EndpointWorkerRuntime,
    pub gpu_cloud_provider_config: C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningComputeType {
    Pod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningStatusEndpoint {
    pub port: u16,
    pub protocol: String,
    pub status_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionerWorkerRuntime {
    pub provisioner_version: String,
    pub docker_image: DockerImage,
    pub volume_mount_path: String,
    pub container_disk_bytes: u64,
    pub compute_type: ProvisioningComputeType,
    pub status_endpoint: ProvisioningStatusEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointWorkerRuntime {
    pub endpoint_worker_version: String,
    pub docker_image: DockerImage,
    pub http_port: u16,
    pub health_path: String,
    pub invoke_path: String,
}
