use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::domain::workspace::ProviderResourceStatus;

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
    #[serde(rename = "storageSupport")]
    pub storage_support: Option<bool>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPodCreateNetworkVolumeRequest {
    pub name: String,
    pub data_center_id: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodNetworkVolumeResponse {
    pub id: Option<String>,
    pub name: Option<String>,
    pub data_center_id: Option<String>,
    pub size: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPodNetworkVolumeObservation {
    pub id: String,
    pub data_center_id: String,
    pub size_gb: u64,
    pub status: ProviderResourceStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPodCreatePodRequest {
    pub name: String,
    pub image_name: String,
    pub gpu_type_ids: Vec<String>,
    pub data_center_ids: Vec<String>,
    pub network_volume_id: String,
    pub volume_mount_path: String,
    pub env: HashMap<String, String>,
    pub ports: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodPodResponse {
    pub id: Option<String>,
    pub name: Option<String>,
    pub data_center_id: Option<String>,
    pub desired_status: Option<String>,
    pub pod_status: Option<String>,
    pub gpu_type_id: Option<String>,
    pub gpu: Option<RunPodPodGpuResponse>,
    pub machine: Option<RunPodPodMachineResponse>,
    pub network_volume_id: Option<String>,
    #[serde(alias = "image")]
    pub image_name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub public_ip: Option<String>,
    pub port_mappings: Option<HashMap<String, u16>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodPodGpuResponse {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodPodMachineResponse {
    pub data_center_id: Option<String>,
    pub gpu_type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPodPodObservation {
    pub id: String,
    pub data_center_id: String,
    pub selected_gpu_id: String,
    pub image_name: String,
    pub status: ProviderResourceStatus,
    pub provisioner_status_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPodCreateTemplateRequest {
    pub name: String,
    pub image_name: String,
    pub container_disk_in_gb: u64,
    pub env: HashMap<String, String>,
    pub is_public: bool,
    pub is_serverless: bool,
    pub ports: Vec<String>,
    pub readme: String,
    pub volume_mount_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodTemplateResponse {
    pub id: Option<String>,
    pub name: Option<String>,
    pub image_name: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub is_serverless: Option<bool>,
    pub ports: Option<Vec<String>>,
    pub volume_mount_path: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPodTemplateObservation {
    pub id: String,
    pub image_name: String,
    pub volume_mount_path: String,
    pub env: HashMap<String, String>,
    pub status: ProviderResourceStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPodCreateEndpointRequest {
    pub name: String,
    pub template_id: String,
    pub gpu_type_ids: Vec<String>,
    pub network_volume_id: String,
    pub data_center_ids: Vec<String>,
    pub workers_min: u32,
    pub workers_max: u32,
    pub scaler_type: String,
    pub scaler_value: u32,
    pub idle_timeout: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunPodEndpointResponse {
    pub id: Option<String>,
    pub name: Option<String>,
    pub template_id: Option<String>,
    pub network_volume_id: Option<String>,
    pub status: Option<String>,
    pub gpu_type_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "optional_string_list")]
    pub data_center_ids: Option<Vec<String>>,
    pub endpoint_url: Option<String>,
    pub idle_timeout: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPodEndpointObservation {
    pub id: String,
    pub data_center_id: String,
    pub selected_gpu_id: String,
    pub status: ProviderResourceStatus,
    pub endpoint_invoke_url: String,
}

fn optional_string_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringList {
        Array(Vec<String>),
        CommaSeparated(String),
    }

    Ok(
        Option::<StringList>::deserialize(deserializer)?.map(|value| match value {
            StringList::Array(values) => values,
            StringList::CommaSeparated(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        }),
    )
}
