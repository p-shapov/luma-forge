use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        placement::{
            RemoteDatacenterPlacementOption, RemoteGpuPlacementOption, RemotePlacementOptions,
        },
        provider::ProviderApiError,
    },
    provisioned_remote::errors::ProvisionedRemoteError,
    secrets_storage::SecretsStorageError,
};

use super::{
    api::{CreateEndpointRequest, CreateNetworkVolumeRequest, CreateProvisionerPodRequest},
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
      available
      stockStatus
    }
  }
}"#;

pub fn not_implemented(operation: &str) -> ProvisionedRemoteError {
    let _ = operation;
    ProviderApiError::RequestFailed.into()
}

pub fn bytes_to_runpod_volume_gb(size_bytes: u64) -> u64 {
    size_bytes.div_ceil(1_000_000_000)
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
    available: bool,
    #[serde(rename = "stockStatus")]
    stock_status: Option<String>,
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
    #[serde(rename = "dataCenterId")]
    datacenter_id: String,
    #[serde(rename = "gpuTypeId")]
    gpu_id: String,
    #[serde(rename = "imageName")]
    image_ref: String,
    #[serde(rename = "networkVolumeId")]
    network_volume_id: String,
    #[serde(rename = "volumeMountPath")]
    mount_path: String,
    name: String,
    ports: String,
    env: Vec<RunpodEnvVar>,
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
    ports: String,
    env: Vec<RunpodEnvVar>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TemplateResponse {
    pub(super) id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EndpointCreateBody {
    #[serde(rename = "dataCenterIds")]
    datacenter_ids: Vec<String>,
    #[serde(rename = "gpuIds")]
    gpu_ids: Vec<String>,
    #[serde(rename = "idleTimeout")]
    idle_timeout: u32,
    name: String,
    #[serde(rename = "networkVolumeId")]
    network_volume_id: String,
    #[serde(rename = "templateId")]
    template_id: String,
    #[serde(rename = "volumeMountPath")]
    mount_path: String,
    #[serde(rename = "workersMax")]
    workers_max: u32,
    #[serde(rename = "workersMin")]
    workers_min: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct EndpointResponse {
    pub(super) id: String,
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct EndpointDetailsResponse {
    #[serde(rename = "templateId")]
    template_id: Option<String>,
    template: Option<EndpointTemplateResponse>,
}

impl EndpointDetailsResponse {
    pub(super) fn template_id(self) -> Result<String, ProvisionedRemoteError> {
        self.template_id
            .or_else(|| self.template.map(|template| template.id))
            .ok_or_else(provider_request_failed)
    }
}

#[derive(Debug, Deserialize)]
struct EndpointTemplateResponse {
    id: String,
}

#[derive(Debug, Serialize)]
struct RunpodEnvVar {
    key: String,
    value: String,
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
    GetEndpoint,
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
    let mut env = vec![RunpodEnvVar {
        key: "LUMA_FORGE_PROVISIONER_TOKEN".to_string(),
        value: request.bearer_token.clone(),
    }];

    if let Some(hugging_face_api_key) = request.hugging_face_api_key.clone() {
        env.push(RunpodEnvVar {
            key: "HF_TOKEN".to_string(),
            value: hugging_face_api_key,
        });
    }

    PodCreateBody {
        datacenter_id: request.datacenter_id.clone(),
        gpu_id: "CPU".to_string(),
        image_ref: request.image_ref.clone(),
        network_volume_id: request.network_volume_id.clone(),
        mount_path: request.mount_path.clone(),
        name: request.name.clone(),
        ports: format!("{PROVISIONER_PORT}/http"),
        env,
    }
}

pub(super) fn endpoint_template_create_body(request: &CreateEndpointRequest) -> TemplateCreateBody {
    TemplateCreateBody {
        image_ref: request.image_ref.clone(),
        name: request.template_name.clone(),
        ports: format!("{PROVISIONER_PORT}/http"),
        env: Vec::new(),
    }
}

pub(super) fn endpoint_create_body(
    request: &CreateEndpointRequest,
    template_id: &str,
) -> EndpointCreateBody {
    EndpointCreateBody {
        datacenter_ids: vec![request.datacenter_id.clone()],
        gpu_ids: vec![request.gpu_id.clone()],
        idle_timeout: request.keep_alive_limits.default_seconds,
        name: request.endpoint_name.clone(),
        network_volume_id: request.network_volume_id.clone(),
        template_id: template_id.to_string(),
        mount_path: request.mount_path.clone(),
        workers_max: 1,
        workers_min: 0,
    }
}

pub(super) async fn parse_json_response<T>(
    response: reqwest::Response,
    operation: RunpodOperation,
) -> Result<T, ProvisionedRemoteError>
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
) -> Result<(), ProvisionedRemoteError> {
    if status.is_success() {
        return Ok(());
    }

    Err(map_status_error(status, operation))
}

fn map_status_error(status: StatusCode, operation: RunpodOperation) -> ProvisionedRemoteError {
    match status {
        StatusCode::UNAUTHORIZED => ProviderApiError::Unauthorized.into(),
        StatusCode::FORBIDDEN => ProviderApiError::InsufficientPermissions.into(),
        StatusCode::TOO_MANY_REQUESTS => ProviderApiError::RateLimited.into(),
        StatusCode::NOT_FOUND => match operation {
            RunpodOperation::DeleteNetworkVolume => ProvisionedRemoteError::RemoteVolumeNotFound,
            RunpodOperation::DeleteProvisionerPod => {
                ProvisionedRemoteError::RemoteProvisionerNotFound
            }
            RunpodOperation::GetEndpoint
            | RunpodOperation::DeleteEndpoint
            | RunpodOperation::DeleteTemplate => ProvisionedRemoteError::RemoteEndpointNotFound,
            _ => provider_request_failed(),
        },
        _ => provider_request_failed(),
    }
}

pub(super) fn map_send_error(error: reqwest::Error) -> ProvisionedRemoteError {
    map_transport_error(error.is_timeout())
}

fn map_transport_error(is_timeout: bool) -> ProvisionedRemoteError {
    if is_timeout {
        ProviderApiError::Timeout.into()
    } else {
        provider_request_failed()
    }
}

fn provider_request_failed() -> ProvisionedRemoteError {
    ProviderApiError::RequestFailed.into()
}

pub(super) fn map_secret_error(error: SecretsStorageError) -> ProvisionedRemoteError {
    match error {
        SecretsStorageError::KeyNotFound => ProvisionedRemoteError::ProviderSecretUnavailable,
        _ => ProviderApiError::RequestFailed.into(),
    }
}

pub(super) fn map_placement_response(
    response: GraphqlResponse<PlacementQueryData>,
) -> Result<RemotePlacementOptions, ProvisionedRemoteError> {
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
                .filter(|availability| availability.available)
                .filter_map(|availability| {
                    data.gpu_types
                        .iter()
                        .find(|gpu| gpu.id == availability.gpu_type_id)
                        .map(|gpu| (availability, gpu))
                })
                .map(|(availability, gpu)| RemoteGpuPlacementOption {
                    id: gpu.id.clone(),
                    name: gpu.display_name.clone(),
                    vram_bytes: gpu.memory_gb * 1_000_000_000,
                    availability_score: stock_status_score(availability.stock_status.as_deref()),
                })
                .collect();

            RemoteDatacenterPlacementOption {
                id: datacenter.id,
                name: datacenter.name,
                gpu_options,
            }
        })
        .collect();

    Ok(RemotePlacementOptions {
        max_persistent_storage_volume_size_bytes: None,
        datacenters,
    })
}

fn stock_status_score(stock_status: Option<&str>) -> u8 {
    match stock_status {
        Some("High") => 100,
        Some("Medium") => 50,
        Some("Low") => 25,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::domain::placement::RemoteEndpointKeepAliveLimits;

    #[test]
    fn bytes_to_runpod_volume_gb_rounds_up_to_decimal_gb() {
        assert_eq!(bytes_to_runpod_volume_gb(0), 0);
        assert_eq!(bytes_to_runpod_volume_gb(1), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_000), 1);
        assert_eq!(bytes_to_runpod_volume_gb(1_000_000_001), 2);
        assert_eq!(bytes_to_runpod_volume_gb(4_000_000_000), 4);
    }

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
            hugging_face_api_key: Some("hf-key".to_string()),
        };

        let body = serde_json::to_value(provisioner_pod_create_body(&request))
            .expect("pod body should serialize");

        assert_eq!(
            body,
            json!({
                "dataCenterId": "US-KS-2",
                "gpuTypeId": "CPU",
                "imageName": "ghcr.io/luma/provisioner:latest",
                "networkVolumeId": "volume-1",
                "volumeMountPath": "/workspace",
                "name": "luma-forge-workspace-provisioner",
                "ports": "8000/http",
                "env": [
                    {"key": "LUMA_FORGE_PROVISIONER_TOKEN", "value": "derived-token"},
                    {"key": "HF_TOKEN", "value": "hf-key"}
                ]
            })
        );
    }

    #[test]
    fn endpoint_creation_serializes_template_before_endpoint_bodies() {
        let request = CreateEndpointRequest {
            datacenter_id: "US-TX-1".to_string(),
            gpu_id: "NVIDIA GeForce RTX 4090".to_string(),
            endpoint_name: "luma-forge-workspace-endpoint".to_string(),
            template_name: "luma-forge-workspace-endpoint-template".to_string(),
            image_ref: "ghcr.io/luma/endpoint:latest".to_string(),
            network_volume_id: "volume-1".to_string(),
            mount_path: "/workspace".to_string(),
            keep_alive_limits: RemoteEndpointKeepAliveLimits {
                default_seconds: 300,
                min_seconds: 0,
                max_seconds: 86_400,
            },
        };

        let template_body = serde_json::to_value(endpoint_template_create_body(&request))
            .expect("template body should serialize");
        let endpoint_body = serde_json::to_value(endpoint_create_body(&request, "template-1"))
            .expect("endpoint body should serialize");

        assert_eq!(
            template_body,
            json!({
                "imageName": "ghcr.io/luma/endpoint:latest",
                "name": "luma-forge-workspace-endpoint-template",
                "ports": "8000/http",
                "env": []
            })
        );
        assert_eq!(
            endpoint_body,
            json!({
                "dataCenterIds": ["US-TX-1"],
                "gpuIds": ["NVIDIA GeForce RTX 4090"],
                "idleTimeout": 300,
                "name": "luma-forge-workspace-endpoint",
                "networkVolumeId": "volume-1",
                "templateId": "template-1",
                "volumeMountPath": "/workspace",
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
    fn endpoint_details_response_resolves_template_id_from_supported_shapes() {
        let with_top_level_id: EndpointDetailsResponse =
            serde_json::from_value(json!({ "templateId": "template-1" }))
                .expect("endpoint should deserialize");
        let with_nested_id: EndpointDetailsResponse =
            serde_json::from_value(json!({ "template": { "id": "template-2" } }))
                .expect("endpoint should deserialize");
        let without_template: EndpointDetailsResponse =
            serde_json::from_value(json!({})).expect("endpoint should deserialize");

        assert_eq!(
            with_top_level_id.template_id(),
            Ok("template-1".to_string())
        );
        assert_eq!(with_nested_id.template_id(), Ok("template-2".to_string()));
        assert_eq!(
            without_template.template_id(),
            Err(ProvisionedRemoteError::ProviderApiFailed(
                ProviderApiError::RequestFailed
            ))
        );
    }

    #[test]
    fn maps_ui_safe_http_and_transport_errors() {
        assert_eq!(
            map_status_error(StatusCode::UNAUTHORIZED, RunpodOperation::CreateEndpoint),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::Unauthorized)
        );
        assert_eq!(
            map_status_error(StatusCode::FORBIDDEN, RunpodOperation::CreateEndpoint),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::InsufficientPermissions)
        );
        assert_eq!(
            map_status_error(
                StatusCode::TOO_MANY_REQUESTS,
                RunpodOperation::CreateEndpoint
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RateLimited)
        );
        assert_eq!(
            map_transport_error(true),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::Timeout)
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteNetworkVolume),
            ProvisionedRemoteError::RemoteVolumeNotFound
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteProvisionerPod),
            ProvisionedRemoteError::RemoteProvisionerNotFound
        );
        assert_eq!(
            map_status_error(StatusCode::NOT_FOUND, RunpodOperation::DeleteEndpoint),
            ProvisionedRemoteError::RemoteEndpointNotFound
        );
        assert_eq!(
            map_status_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                RunpodOperation::CreateEndpoint
            ),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RequestFailed)
        );
        assert_eq!(
            map_transport_error(false),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RequestFailed)
        );
        assert_eq!(
            map_secret_error(SecretsStorageError::KeyNotFound),
            ProvisionedRemoteError::ProviderSecretUnavailable
        );
        assert_eq!(
            map_secret_error(SecretsStorageError::StoreUnavailable),
            ProvisionedRemoteError::ProviderApiFailed(ProviderApiError::RequestFailed)
        );
    }

    #[test]
    fn placement_response_maps_available_gpu_options() {
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
                        available: true,
                        stock_status: Some("Low".to_string()),
                    }],
                }],
            }),
            errors: Vec::new(),
        };

        let options = map_placement_response(response).expect("placement response should map");

        assert_eq!(
            options,
            RemotePlacementOptions {
                max_persistent_storage_volume_size_bytes: None,
                datacenters: vec![RemoteDatacenterPlacementOption {
                    id: "US-TX-1".to_string(),
                    name: "Texas".to_string(),
                    gpu_options: vec![RemoteGpuPlacementOption {
                        id: "gpu-1".to_string(),
                        name: "RTX 4090".to_string(),
                        vram_bytes: 24_000_000_000,
                        availability_score: 25,
                    }],
                }],
            }
        );
    }
}
