use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodIdentity {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodPlacementOptions {
    pub gpu_types: Vec<RunpodGpuType>,
    pub datacenters: Vec<RunpodDatacenter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuType {
    pub id: String,
    pub display_name: String,
    pub memory_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodDatacenter {
    pub id: String,
    pub name: String,
    pub gpu_availability: Vec<RunpodGpuAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuAvailability {
    pub gpu_type_id: String,
    pub available: Option<bool>,
    pub stock_status: Option<String>,
}

#[derive(Serialize)]
pub struct CreateNetworkVolumeRequest {
    #[serde(rename = "dataCenterId")]
    pub datacenter_id: String,
    pub name: String,
    #[serde(rename = "size")]
    pub size_gb: u64,
}

#[derive(Clone, Copy, Serialize)]
pub enum RunpodComputeType {
    #[serde(rename = "CPU")]
    Cpu,
    #[serde(rename = "GPU")]
    Gpu,
}

#[derive(Serialize)]
pub struct CreatePodRequest {
    #[serde(rename = "dataCenterIds")]
    pub datacenter_ids: Vec<String>,
    #[serde(rename = "computeType")]
    pub compute_type: RunpodComputeType,
    #[serde(rename = "gpuTypeIds")]
    pub gpu_type_ids: Vec<String>,
    #[serde(rename = "imageName")]
    pub image_name: String,
    #[serde(rename = "networkVolumeId")]
    pub network_volume_id: String,
    pub name: String,
    pub ports: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct CreateTemplateRequest {
    #[serde(rename = "imageName")]
    pub image_name: String,
    pub name: String,
    #[serde(rename = "isServerless")]
    pub is_serverless: bool,
}

#[derive(Serialize)]
pub struct CreateEndpointRequest {
    #[serde(rename = "dataCenterIds")]
    pub datacenter_ids: Vec<String>,
    #[serde(rename = "gpuTypeIds")]
    pub gpu_type_ids: Vec<String>,
    pub name: String,
    #[serde(rename = "networkVolumeId")]
    pub network_volume_id: String,
    #[serde(rename = "templateId")]
    pub template_id: String,
    #[serde(rename = "workersMin")]
    pub workers_min: u32,
    #[serde(rename = "workersMax")]
    pub workers_max: u32,
}
