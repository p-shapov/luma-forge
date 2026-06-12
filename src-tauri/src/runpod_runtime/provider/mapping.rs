use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    domain::runpod::{
        RunpodDatacenterPlacementOption, RunpodGpuPlacementOption, RunpodPlacementOptions,
    },
    runpod_runtime::errors::{ProviderApiError, RunpodRuntimeError},
    secrets_storage::SecretsStorageError,
};

use super::{
    api::{
        CreateNetworkVolumeRequest, CreateProvisionerPodRequest, CreateServerlessEndpointRequest,
        CreateServerlessTemplateRequest,
    },
    config::PROVISIONER_PORT,
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

pub fn not_implemented(operation: &str) -> RunpodRuntimeError {
    let _ = operation;
    ProviderApiError::RequestFailed.into()
}

pub fn workspace_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
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
    _message: String,
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
    #[serde(rename = "volumeMountPath")]
    mount_path: String,
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
    ports: Vec<String>,
    env: HashMap<String, String>,
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
    #[serde(rename = "idleTimeout")]
    idle_timeout: u32,
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
    pub(super) url: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RunpodOperation {
    PlacementOptions,
    CreateNetworkVolume,
    DeleteNetworkVolume,
    CreateProvisionerPod,
    DeleteProvisionerPod,
    CreateTemplate,
    DeleteTemplate,
    CreateEndpoint,
    DeleteEndpoint,
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

pub(super) fn provisioner_pod_create_body(request: &CreateProvisionerPodRequest) -> PodCreateBody {
    let mut env = HashMap::from([
        (
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
            request.bearer_token.clone(),
        ),
        (
            "LUMA_FORGE_PROVISIONER_JOB_ID".to_string(),
            request.job_id.clone(),
        ),
        (
            "LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY".to_string(),
            request.requires_hugging_face_api_key.clone(),
        ),
        (
            "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS".to_string(),
            request.required_model_assets.clone(),
        ),
    ]);

    if let Some(hugging_face_api_key) = request.hugging_face_api_key.clone() {
        env.insert(
            "LUMA_FORGE_HUGGING_FACE_API_KEY".to_string(),
            hugging_face_api_key,
        );
    }

    PodCreateBody {
        datacenter_ids: vec![request.datacenter_id.clone()],
        compute_type: "CPU".to_string(),
        gpu_type_ids: Vec::new(),
        image_ref: request.image_ref.clone(),
        network_volume_id: request.network_volume_id.clone(),
        mount_path: request.mount_path.clone(),
        name: request.name.clone(),
        ports: vec![format!("{PROVISIONER_PORT}/http")],
        env,
    }
}

pub(super) fn endpoint_template_create_body(
    request: &CreateServerlessTemplateRequest,
) -> TemplateCreateBody {
    let mut env = HashMap::new();
    env.insert(
        "LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH".to_string(),
        request.mount_path.clone(),
    );

    TemplateCreateBody {
        image_ref: request.image_ref.clone(),
        name: request.name.clone(),
        is_serverless: true,
        ports: vec![format!("{PROVISIONER_PORT}/http")],
        env,
    }
}

pub(super) fn endpoint_create_body(
    request: &CreateServerlessEndpointRequest,
) -> EndpointCreateBody {
    EndpointCreateBody {
        datacenter_ids: vec![request.datacenter_id.clone()],
        gpu_type_ids: vec![request.gpu_id.clone()],
        idle_timeout: request.keep_alive_limits.default_seconds,
        name: request.name.clone(),
        network_volume_id: request.network_volume_id.clone(),
        template_id: request.template_id.clone(),
        workers_max: 1,
        workers_min: 0,
    }
}

pub(super) async fn parse_json_response<T>(
    response: reqwest::Response,
    operation: RunpodOperation,
) -> Result<T, RunpodRuntimeError>
where
    T: for<'de> Deserialize<'de>,
{
    map_empty_response(response.status(), operation)?;
    response
        .json::<T>()
        .await
        .map_err(|_| provider_request_failed())
}

pub(super) fn map_empty_response(
    status: StatusCode,
    operation: RunpodOperation,
) -> Result<(), RunpodRuntimeError> {
    if status.is_success() {
        return Ok(());
    }

    Err(map_status_error(status, operation))
}

fn map_status_error(status: StatusCode, operation: RunpodOperation) -> RunpodRuntimeError {
    match status {
        StatusCode::UNAUTHORIZED => ProviderApiError::Unauthorized.into(),
        StatusCode::FORBIDDEN => ProviderApiError::InsufficientPermissions.into(),
        StatusCode::TOO_MANY_REQUESTS => ProviderApiError::RateLimited.into(),
        StatusCode::NOT_FOUND => match operation {
            RunpodOperation::DeleteNetworkVolume => RunpodRuntimeError::NetworkVolumeNotFound,
            RunpodOperation::DeleteProvisionerPod => RunpodRuntimeError::ProvisionerPodNotFound,
            RunpodOperation::DeleteEndpoint => RunpodRuntimeError::EndpointNotFound,
            RunpodOperation::DeleteTemplate => RunpodRuntimeError::TemplateNotFound,
            _ => provider_request_failed(),
        },
        _ => provider_request_failed(),
    }
}

pub(super) fn map_send_error(error: reqwest::Error) -> RunpodRuntimeError {
    map_transport_error(error.is_timeout())
}

fn map_transport_error(is_timeout: bool) -> RunpodRuntimeError {
    if is_timeout {
        ProviderApiError::Timeout.into()
    } else {
        provider_request_failed()
    }
}

fn provider_request_failed() -> RunpodRuntimeError {
    ProviderApiError::RequestFailed.into()
}

pub(super) fn map_secret_error(error: SecretsStorageError) -> RunpodRuntimeError {
    match error {
        SecretsStorageError::KeyNotFound => RunpodRuntimeError::RunpodSecretUnavailable,
        _ => ProviderApiError::RequestFailed.into(),
    }
}

pub(super) fn map_placement_response(
    response: GraphqlResponse<PlacementQueryData>,
) -> Result<RunpodPlacementOptions, RunpodRuntimeError> {
    if !response.errors.is_empty() {
        return Err(provider_request_failed());
    }

    let data = response.data.ok_or_else(provider_request_failed)?;
    let datacenters = data
        .datacenters
        .into_iter()
        .map(|datacenter| {
            let gpu_options = datacenter
                .gpu_availability
                .into_iter()
                .filter_map(|availability| {
                    data.gpu_types
                        .iter()
                        .find(|gpu| gpu.id == availability.gpu_type_id)
                        .map(|gpu| (availability, gpu))
                })
                .map(|(_, gpu)| RunpodGpuPlacementOption {
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
        })
        .collect();

    Ok(RunpodPlacementOptions {
        max_volume_size_gb: None,
        datacenters,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::runpod_runtime::provider::config::ENDPOINT_WORKSPACE_MOUNT_PATH;
    use crate::runpod_runtime::provider::RunpodEndpointKeepAliveLimits;

    #[test]
    fn workspace_resource_name_is_deterministic() {
        assert_eq!(
            workspace_resource_name("workspace-1", "volume"),
            "luma-forge-workspace-1-volume"
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
            mount_path: "/workspace".to_string(),
            bearer_token: "derived-token".to_string(),
            job_id: "job-1".to_string(),
            requires_hugging_face_api_key: "false".to_string(),
            required_model_assets: r#"[{"id":"model","name":"Model","download_source":{"source_type":"huggingface","repository_id":"owner/model","file_path":"model.safetensors","revision":"main"},"install_comfyui_relative_path":"models/checkpoints/model.safetensors"}]"#.to_string(),
            hugging_face_api_key: Some("hf-key".to_string()),
        };

        let body = serde_json::to_value(provisioner_pod_create_body(&request))
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
        assert_eq!(body["volumeMountPath"], json!("/workspace"));
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
        assert_eq!(body["env"]["LUMA_FORGE_PROVISIONER_JOB_ID"], json!("job-1"));
        assert_eq!(
            body["env"]["LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY"],
            json!("false")
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
            mount_path: ENDPOINT_WORKSPACE_MOUNT_PATH.to_string(),
        };
        let endpoint_request = CreateServerlessEndpointRequest {
            datacenter_id: "US-TX-1".to_string(),
            gpu_id: "NVIDIA GeForce RTX 4090".to_string(),
            name: "luma-forge-workspace-endpoint".to_string(),
            template_id: "template-1".to_string(),
            network_volume_id: "volume-1".to_string(),
            keep_alive_limits: RunpodEndpointKeepAliveLimits {
                default_seconds: 300,
                min_seconds: 0,
                max_seconds: 86_400,
            },
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
                "isServerless": true,
                "ports": ["8000/http"],
                "env": {
                    "LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH": "/runpod-volume"
                }
            })
        );
        assert_eq!(
            endpoint_body,
            json!({
                "dataCenterIds": ["US-TX-1"],
                "gpuTypeIds": ["NVIDIA GeForce RTX 4090"],
                "idleTimeout": 300,
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
    fn endpoint_response_preserves_id_and_url_without_template_id() {
        let response: EndpointResponse = serde_json::from_value(json!({
            "id": "endpoint-1",
            "url": "https://endpoint.example",
        }))
        .expect("endpoint should deserialize");

        assert_eq!(response.id, "endpoint-1");
        assert_eq!(response.url, Some("https://endpoint.example".to_string()));
    }

    #[test]
    fn maps_ui_safe_http_and_transport_errors() {
        assert_eq!(
            map_status_error(StatusCode::UNAUTHORIZED, RunpodOperation::CreateEndpoint),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::Unauthorized)
        );
        assert_eq!(
            map_status_error(StatusCode::FORBIDDEN, RunpodOperation::CreateEndpoint),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::InsufficientPermissions)
        );
        assert_eq!(
            map_status_error(
                StatusCode::TOO_MANY_REQUESTS,
                RunpodOperation::CreateEndpoint
            ),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::RateLimited)
        );
        assert_eq!(
            map_transport_error(true),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::Timeout)
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteNetworkVolume),
            RunpodRuntimeError::NetworkVolumeNotFound
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteProvisionerPod),
            RunpodRuntimeError::ProvisionerPodNotFound
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteEndpoint),
            RunpodRuntimeError::EndpointNotFound
        );
        assert_eq!(
            map_status_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                RunpodOperation::CreateEndpoint
            ),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::RequestFailed)
        );
        assert_eq!(
            map_transport_error(false),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::RequestFailed)
        );
        assert_eq!(
            map_secret_error(SecretsStorageError::KeyNotFound),
            RunpodRuntimeError::RunpodSecretUnavailable
        );
        assert_eq!(
            map_secret_error(SecretsStorageError::StoreUnavailable),
            RunpodRuntimeError::RunpodApiFailed(ProviderApiError::RequestFailed)
        );
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
