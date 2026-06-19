use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    domain::runpod::{
        RunpodDatacenterPlacementOption, RunpodGpuPlacementOption, RunpodPlacementOptions,
    },
    domain::workflow_preset::ModelAsset,
    shared::{map_api_status_error, map_api_transport_error, ApiError},
};

const PROVISIONER_COMPUTE_TYPE: &str = "CPU";
const WORKER_PORT_PROTOCOL: &str = "http";
const ENDPOINT_WORKERS_MIN: u32 = 0;
const ENDPOINT_WORKERS_MAX: u32 = 1;
const ENV_PROVISIONER_BEARER_TOKEN: &str = "LUMA_FORGE_PROVISIONER_BEARER_TOKEN";
const ENV_PROVISIONER_REQUIRED_MODEL_ASSETS: &str = "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS";
const ENV_HUGGING_FACE_API_KEY: &str = "LUMA_FORGE_HUGGING_FACE_API_KEY";
const PROVISIONER_PORT: u16 = 8000;

const RUNPOD_PLACEMENT_QUERY: &str = r#"query LumaForgeRunpodPlacementOptions {
  gpuTypes {
    id
    displayName
    memoryInGb
  }
  dataCenters {
    id
    name
    gpuAvailability {
      gpuTypeId
      stockStatus
    }
  }
}"#;
const RESOURCE_PREFIX: &str = "luma-forge";
const NETWORK_VOLUME_SUFFIX: &str = "volume";
const PROVISIONER_POD_SUFFIX: &str = "provisioner";
const ENDPOINT_TEMPLATE_SUFFIX: &str = "endpoint-template";
const ENDPOINT_SUFFIX: &str = "endpoint";

pub(super) fn network_volume_name(workspace_id: &str) -> String {
    workspace_resource_name(workspace_id, NETWORK_VOLUME_SUFFIX)
}

pub(super) fn provisioner_pod_name(workspace_id: &str) -> String {
    workspace_resource_name(workspace_id, PROVISIONER_POD_SUFFIX)
}

pub(super) fn endpoint_template_name(workspace_id: &str) -> String {
    workspace_resource_name(workspace_id, ENDPOINT_TEMPLATE_SUFFIX)
}

pub(super) fn endpoint_name(workspace_id: &str) -> String {
    workspace_resource_name(workspace_id, ENDPOINT_SUFFIX)
}

fn workspace_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("{RESOURCE_PREFIX}-{workspace_id}-{suffix}")
}

#[derive(Serialize)]
pub(super) struct GraphqlRequest {
    query: &'static str,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlResponse<T> {
    pub(super) data: Option<T>,
    #[serde(default)]
    pub(super) errors: Vec<GraphqlError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlError {
    #[serde(rename = "message")]
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PlacementQueryData {
    #[serde(rename = "gpuTypes")]
    gpu_types: Vec<PlacementGpuType>,
    #[serde(rename = "dataCenters")]
    datacenters: Vec<PlacementDatacenter>,
}

#[derive(Debug, Deserialize)]
struct PlacementGpuType {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "memoryInGb")]
    memory_gb: u64,
}

#[derive(Debug, Deserialize)]
struct PlacementDatacenter {
    id: String,
    name: String,
    #[serde(rename = "gpuAvailability")]
    gpu_availability: Vec<PlacementGpuAvailability>,
}

#[derive(Debug, Deserialize)]
struct PlacementGpuAvailability {
    #[serde(rename = "gpuTypeId")]
    gpu_type_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct NetworkVolumeCreateBody {
    #[serde(rename = "dataCenterId")]
    datacenter_id: String,
    name: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct NetworkVolumeResponse {
    pub(super) id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct PodCreateBody {
    #[serde(rename = "dataCenterIds")]
    datacenter_ids: Vec<String>,
    #[serde(rename = "computeType")]
    compute_type: String,
    #[serde(rename = "gpuTypeIds")]
    gpu_type_ids: Vec<String>,
    #[serde(rename = "imageName")]
    image_ref: String,
    #[serde(rename = "networkVolumeId")]
    network_volume_id: String,
    name: String,
    ports: Vec<String>,
    env: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PodResponse {
    pub(super) id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TemplateCreateBody {
    #[serde(rename = "imageName")]
    image_ref: String,
    name: String,
    #[serde(rename = "isServerless")]
    is_serverless: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct TemplateResponse {
    pub(super) id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EndpointCreateBody {
    #[serde(rename = "dataCenterIds")]
    datacenter_ids: Vec<String>,
    #[serde(rename = "gpuTypeIds")]
    gpu_type_ids: Vec<String>,
    name: String,
    #[serde(rename = "networkVolumeId")]
    network_volume_id: String,
    #[serde(rename = "templateId")]
    template_id: String,
    #[serde(rename = "workersMax")]
    workers_max: u32,
    #[serde(rename = "workersMin")]
    workers_min: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct EndpointResponse {
    pub(super) id: String,
}

pub(super) fn placement_graphql_request() -> GraphqlRequest {
    GraphqlRequest {
        query: RUNPOD_PLACEMENT_QUERY,
    }
}

pub(super) fn network_volume_create_body(
    workspace_id: &str,
    datacenter_id: String,
    size_gb: u64,
) -> NetworkVolumeCreateBody {
    NetworkVolumeCreateBody {
        datacenter_id,
        name: network_volume_name(workspace_id),
        size: size_gb,
    }
}

pub(super) fn provisioner_pod_create_body(
    workspace_id: &str,
    datacenter_id: String,
    image_ref: String,
    network_volume_id: String,
    bearer_token: String,
    required_model_assets: Vec<ModelAsset>,
    hugging_face_api_key: Option<String>,
) -> Result<PodCreateBody, ApiError> {
    let required_model_assets =
        serde_json::to_string(&required_model_assets).map_err(|_| provider_request_failed())?;
    let mut env = HashMap::from([
        (ENV_PROVISIONER_BEARER_TOKEN.to_string(), bearer_token),
        (
            ENV_PROVISIONER_REQUIRED_MODEL_ASSETS.to_string(),
            required_model_assets,
        ),
    ]);

    if let Some(hugging_face_api_key) = hugging_face_api_key {
        env.insert(ENV_HUGGING_FACE_API_KEY.to_string(), hugging_face_api_key);
    }

    Ok(PodCreateBody {
        datacenter_ids: vec![datacenter_id],
        compute_type: PROVISIONER_COMPUTE_TYPE.to_string(),
        gpu_type_ids: Vec::new(),
        image_ref,
        network_volume_id,
        name: provisioner_pod_name(workspace_id),
        ports: vec![worker_port()],
        env,
    })
}

pub(super) fn endpoint_template_create_body(
    workspace_id: &str,
    image_ref: String,
) -> TemplateCreateBody {
    TemplateCreateBody {
        image_ref,
        name: endpoint_template_name(workspace_id),
        is_serverless: true,
    }
}

pub(super) fn endpoint_create_body(
    workspace_id: &str,
    datacenter_id: String,
    gpu_id: String,
    network_volume_id: String,
    template_id: String,
) -> EndpointCreateBody {
    EndpointCreateBody {
        datacenter_ids: vec![datacenter_id],
        gpu_type_ids: vec![gpu_id],
        name: endpoint_name(workspace_id),
        network_volume_id,
        template_id,
        workers_max: ENDPOINT_WORKERS_MAX,
        workers_min: ENDPOINT_WORKERS_MIN,
    }
}

fn worker_port() -> String {
    format!("{PROVISIONER_PORT}/{WORKER_PORT_PROTOCOL}")
}

pub(super) async fn parse_json_response<T>(response: reqwest::Response) -> Result<T, ApiError>
where
    T: for<'de> Deserialize<'de>,
{
    map_empty_response(response.status())?;
    response
        .json::<T>()
        .await
        .map_err(|_| provider_request_failed())
}

pub(super) fn map_empty_response(status: StatusCode) -> Result<(), ApiError> {
    if status.is_success() {
        return Ok(());
    }

    Err(map_status_error(status))
}

fn map_status_error(status: StatusCode) -> ApiError {
    map_api_status_error("RunPod", status, |error| error).unwrap_or_else(provider_request_failed)
}

pub(super) fn map_send_error(error: reqwest::Error) -> ApiError {
    map_api_transport_error(error, |error| error)
}

fn provider_request_failed() -> ApiError {
    ApiError::RequestFailed {
        message: "RunPod API request failed".to_string(),
    }
}

pub(super) fn map_placement_response(
    response: GraphqlResponse<PlacementQueryData>,
) -> Result<RunpodPlacementOptions, ApiError> {
    if !response.errors.is_empty() {
        let message = response
            .errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ApiError::RequestFailed {
            message: crate::diagnostics::redact_for_log(&message),
        });
    }

    let data = response.data.ok_or_else(provider_request_failed)?;
    let gpu_types_by_id: HashMap<&str, &PlacementGpuType> = data
        .gpu_types
        .iter()
        .map(|gpu| (gpu.id.as_str(), gpu))
        .collect();
    let datacenters = data
        .datacenters
        .into_iter()
        .map(|datacenter| map_datacenter_placement(datacenter, &gpu_types_by_id))
        .collect();

    Ok(RunpodPlacementOptions {
        max_volume_size_gb: None,
        datacenters,
    })
}

fn map_datacenter_placement(
    datacenter: PlacementDatacenter,
    gpu_types_by_id: &HashMap<&str, &PlacementGpuType>,
) -> RunpodDatacenterPlacementOption {
    let gpu_options = datacenter
        .gpu_availability
        .into_iter()
        .filter_map(|availability| gpu_types_by_id.get(availability.gpu_type_id.as_str()))
        .map(|gpu| RunpodGpuPlacementOption {
            id: gpu.id.clone(),
            name: gpu.display_name.clone(),
            vram_gb: gpu.memory_gb,
        })
        .collect();

    RunpodDatacenterPlacementOption {
        id: datacenter.id,
        name: datacenter.name,
        gpu_options,
    }
}
