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

use super::config::{
    ENDPOINT_WORKERS_MAX, ENDPOINT_WORKERS_MIN, ENV_HUGGING_FACE_API_KEY,
    ENV_PROVISIONER_BEARER_TOKEN, ENV_PROVISIONER_REQUIRED_MODEL_ASSETS, PROVISIONER_COMPUTE_TYPE,
    PROVISIONER_PORT, WORKER_PORT_PROTOCOL,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreateNetworkVolumeRequest {
    pub(super) datacenter_id: String,
    pub(super) name: String,
    pub(super) size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreateProvisionerPodRequest {
    pub(super) datacenter_id: String,
    pub(super) name: String,
    pub(super) image_ref: String,
    pub(super) network_volume_id: String,
    pub(super) bearer_token: String,
    pub(super) required_model_assets: Vec<ModelAsset>,
    pub(super) hugging_face_api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreateServerlessTemplateRequest {
    pub(super) name: String,
    pub(super) image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CreateServerlessEndpointRequest {
    pub(super) datacenter_id: String,
    pub(super) gpu_id: String,
    pub(super) name: String,
    pub(super) template_id: String,
    pub(super) network_volume_id: String,
}

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
pub(super) struct GraphqlRequest<V> {
    query: &'static str,
    variables: V,
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

pub(super) fn placement_graphql_request() -> GraphqlRequest<serde_json::Value> {
    GraphqlRequest {
        query: RUNPOD_PLACEMENT_QUERY,
        variables: serde_json::json!({}),
    }
}

pub(super) fn network_volume_create_body(
    request: &CreateNetworkVolumeRequest,
) -> NetworkVolumeCreateBody {
    NetworkVolumeCreateBody {
        datacenter_id: request.datacenter_id.clone(),
        name: request.name.clone(),
        size: request.size_gb,
    }
}

pub(super) fn provisioner_pod_create_body(
    request: &CreateProvisionerPodRequest,
) -> Result<PodCreateBody, ApiError> {
    let required_model_assets = serde_json::to_string(&request.required_model_assets)
        .map_err(|_| provider_request_failed())?;
    let mut env = HashMap::from([
        (
            ENV_PROVISIONER_BEARER_TOKEN.to_string(),
            request.bearer_token.clone(),
        ),
        (
            ENV_PROVISIONER_REQUIRED_MODEL_ASSETS.to_string(),
            required_model_assets,
        ),
    ]);

    if let Some(hugging_face_api_key) = request.hugging_face_api_key.clone() {
        env.insert(ENV_HUGGING_FACE_API_KEY.to_string(), hugging_face_api_key);
    }

    Ok(PodCreateBody {
        datacenter_ids: vec![request.datacenter_id.clone()],
        compute_type: PROVISIONER_COMPUTE_TYPE.to_string(),
        gpu_type_ids: Vec::new(),
        image_ref: request.image_ref.clone(),
        network_volume_id: request.network_volume_id.clone(),
        name: request.name.clone(),
        ports: vec![worker_port()],
        env,
    })
}

pub(super) fn endpoint_template_create_body(
    request: &CreateServerlessTemplateRequest,
) -> TemplateCreateBody {
    TemplateCreateBody {
        image_ref: request.image_ref.clone(),
        name: request.name.clone(),
        is_serverless: true,
    }
}

pub(super) fn endpoint_create_body(
    request: &CreateServerlessEndpointRequest,
) -> EndpointCreateBody {
    EndpointCreateBody {
        datacenter_ids: vec![request.datacenter_id.clone()],
        gpu_type_ids: vec![request.gpu_id.clone()],
        name: request.name.clone(),
        network_volume_id: request.network_volume_id.clone(),
        template_id: request.template_id.clone(),
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::domain::workflow_preset::{ModelAsset, ModelAssetSource};

    #[test]
    fn workspace_resource_names_are_deterministic() {
        assert_eq!(
            network_volume_name("workspace-1"),
            "luma-forge-workspace-1-volume"
        );
        assert_eq!(
            provisioner_pod_name("workspace-1"),
            "luma-forge-workspace-1-provisioner"
        );
        assert_eq!(
            endpoint_template_name("workspace-1"),
            "luma-forge-workspace-1-endpoint-template"
        );
        assert_eq!(
            endpoint_name("workspace-1"),
            "luma-forge-workspace-1-endpoint"
        );
    }

    #[test]
    fn network_volume_create_serializes_datacenter_name_and_gb_size() {
        let request = CreateNetworkVolumeRequest {
            datacenter_id: "EU-RO-1".to_string(),
            name: "luma-forge-workspace-volume".to_string(),
            size_gb: 75,
        };

        let body = serde_json::to_value(network_volume_create_body(&request))
            .expect("network volume body should serialize");

        assert_eq!(
            body,
            json!({
                "dataCenterId": "EU-RO-1",
                "name": "luma-forge-workspace-volume",
                "size": 75
            })
        );
    }

    #[test]
    fn provisioner_pod_create_serializes_cpu_volume_port_and_env() {
        let request = CreateProvisionerPodRequest {
            datacenter_id: "US-KS-2".to_string(),
            name: "luma-forge-workspace-provisioner".to_string(),
            image_ref: "ghcr.io/luma/provisioner:latest".to_string(),
            network_volume_id: "volume-1".to_string(),
            bearer_token: "derived-token".to_string(),
            required_model_assets: vec![ModelAsset {
                id: "model".to_string(),
                name: "Model".to_string(),
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/model".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                install_comfyui_relative_path: "models/checkpoints/model.safetensors".to_string(),
            }],
            hugging_face_api_key: Some("hf-key".to_string()),
        };

        let body = serde_json::to_value(
            provisioner_pod_create_body(&request).expect("pod body should build"),
        )
        .expect("pod body should serialize");
        let expected_required_model_assets = json!([
            {
                "id": "model",
                "name": "Model",
                "download_source": {
                    "source_type": "huggingface",
                    "repository_id": "owner/model",
                    "file_path": "model.safetensors",
                    "revision": "main",
                },
                "install_comfyui_relative_path": "models/checkpoints/model.safetensors",
            },
        ]);

        assert_eq!(body["dataCenterIds"], json!(["US-KS-2"]));
        assert_eq!(body["computeType"], json!("CPU"));
        assert_eq!(body["gpuTypeIds"], json!([]));
        assert_eq!(body["imageName"], json!("ghcr.io/luma/provisioner:latest"));
        assert_eq!(body["networkVolumeId"], json!("volume-1"));
        assert_eq!(body["name"], json!("luma-forge-workspace-provisioner"));
        assert_eq!(body["ports"], json!(["8000/http"]));
        assert_eq!(
            body["env"]["LUMA_FORGE_PROVISIONER_BEARER_TOKEN"],
            json!("derived-token")
        );
        assert_eq!(
            body["env"]["LUMA_FORGE_HUGGING_FACE_API_KEY"],
            json!("hf-key")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                body["env"]["LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS"]
                    .as_str()
                    .unwrap(),
            )
            .expect("required model assets should decode"),
            expected_required_model_assets
        );
    }

    #[test]
    fn endpoint_creation_serializes_template_before_endpoint_bodies() {
        let template_request = CreateServerlessTemplateRequest {
            name: "luma-forge-workspace-endpoint-template".to_string(),
            image_ref: "ghcr.io/luma/endpoint:latest".to_string(),
        };
        let endpoint_request = CreateServerlessEndpointRequest {
            datacenter_id: "US-TX-1".to_string(),
            gpu_id: "NVIDIA GeForce RTX 4090".to_string(),
            name: "luma-forge-workspace-endpoint".to_string(),
            template_id: "template-1".to_string(),
            network_volume_id: "volume-1".to_string(),
        };

        let template_body = serde_json::to_value(endpoint_template_create_body(&template_request))
            .expect("template body should serialize");
        let endpoint_body = serde_json::to_value(endpoint_create_body(&endpoint_request))
            .expect("endpoint body should serialize");

        assert_eq!(
            template_body,
            json!({
                "imageName": "ghcr.io/luma/endpoint:latest",
                "name": "luma-forge-workspace-endpoint-template",
                "isServerless": true
            })
        );
        assert_eq!(
            endpoint_body,
            json!({
                "dataCenterIds": ["US-TX-1"],
                "gpuTypeIds": ["NVIDIA GeForce RTX 4090"],
                "name": "luma-forge-workspace-endpoint",
                "networkVolumeId": "volume-1",
                "templateId": "template-1",
                "workersMax": 1,
                "workersMin": 0
            })
        );
    }

    #[test]
    fn placement_graphql_request_uses_constant_query_and_empty_variables() {
        let body = serde_json::to_value(placement_graphql_request())
            .expect("graphql body should serialize");

        assert_eq!(
            body,
            json!({
                "query": RUNPOD_PLACEMENT_QUERY,
                "variables": {}
            })
        );
    }

    #[test]
    fn endpoint_response_preserves_id() {
        let response: EndpointResponse = serde_json::from_value(json!({
            "id": "endpoint-1",
        }))
        .expect("endpoint should deserialize");

        assert_eq!(response.id, "endpoint-1");
    }

    #[test]
    fn placement_response_preserves_runpod_gb_units() {
        let response = GraphqlResponse {
            data: Some(PlacementQueryData {
                gpu_types: vec![PlacementGpuType {
                    id: "gpu-1".to_string(),
                    display_name: "RTX 4090".to_string(),
                    memory_gb: 24,
                }],
                datacenters: vec![PlacementDatacenter {
                    id: "US-TX-1".to_string(),
                    name: "Texas".to_string(),
                    gpu_availability: vec![PlacementGpuAvailability {
                        gpu_type_id: "gpu-1".to_string(),
                    }],
                }],
            }),
            errors: Vec::new(),
        };

        let options = map_placement_response(response).expect("placement response should map");

        assert_eq!(
            options,
            RunpodPlacementOptions {
                max_volume_size_gb: None,
                datacenters: vec![RunpodDatacenterPlacementOption {
                    id: "US-TX-1".to_string(),
                    name: "Texas".to_string(),
                    gpu_options: vec![RunpodGpuPlacementOption {
                        id: "gpu-1".to_string(),
                        name: "RTX 4090".to_string(),
                        vram_gb: 24,
                    }],
                }],
            }
        );
    }
}
