use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(super) struct GraphQlRequest<'a> {
    pub query: &'a str,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodIdentityData {
    pub myself: Option<RunPodUser>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodUser {
    pub email: Option<String>,
    #[serde(rename = "apiKeys")]
    pub api_keys: Option<Vec<RunPodApiKey>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(super) struct RunPodApiKey {
    pub id: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodInventoryData {
    #[serde(rename = "dataCenters")]
    pub data_centers: Option<Vec<RunPodInventoryDatacenter>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodInventoryDatacenter {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "gpuAvailability")]
    pub gpu_availability: Option<Vec<RunPodGpuAvailability>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodGpuAvailability {
    #[serde(rename = "stockStatus")]
    pub stock_status: Option<String>,
    #[serde(rename = "gpuType")]
    pub gpu_type: Option<RunPodGpuType>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunPodGpuType {
    pub id: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "memoryInGb")]
    pub memory_in_gb: Option<u64>,
}
