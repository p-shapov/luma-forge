use std::time::Duration;

use reqwest::{StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::RunpodError;

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const IDENTITY_QUERY: &str = "query LumaForgeRunpodIdentity { myself { id email } }";
const PLACEMENT_QUERY: &str = r#"query LumaForgeRunpodPlacement {
  gpuTypes {
    id
    displayName
    memoryInGb
  }
  myself {
    datacenters {
      id
      name
      gpuAvailability {
        gpuTypeId
        available
        stockStatus
      }
    }
  }
}"#;

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

#[derive(Clone)]
pub struct RunpodClient {
    http: reqwest::Client,
}

impl RunpodClient {
    pub fn new() -> Result<Self, RunpodError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| RunpodError::RequestFailed)?;

        Ok(Self { http })
    }

    pub async fn identity(&self, api_key: &SecretString) -> Result<RunpodIdentity, RunpodError> {
        map_identity(self.graphql(api_key, IDENTITY_QUERY).await?)
    }

    pub async fn placement_options(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacementOptions, RunpodError> {
        map_placement(self.graphql(api_key, PLACEMENT_QUERY).await?)
    }

    pub async fn create_network_volume(
        &self,
        api_key: &SecretString,
        request: CreateNetworkVolumeRequest,
    ) -> Result<String, RunpodError> {
        self.create_resource(api_key, "networkvolumes", &request)
            .await
    }

    pub async fn delete_network_volume(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodError> {
        self.delete_resource(api_key, "networkvolumes", id).await
    }

    pub async fn create_pod(
        &self,
        api_key: &SecretString,
        request: CreatePodRequest,
    ) -> Result<String, RunpodError> {
        self.create_resource(api_key, "pods", &request).await
    }

    pub async fn delete_pod(&self, api_key: &SecretString, id: &str) -> Result<(), RunpodError> {
        self.delete_resource(api_key, "pods", id).await
    }

    pub async fn create_template(
        &self,
        api_key: &SecretString,
        request: CreateTemplateRequest,
    ) -> Result<String, RunpodError> {
        self.create_resource(api_key, "templates", &request).await
    }

    pub async fn delete_template(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodError> {
        self.delete_resource(api_key, "templates", id).await
    }

    pub async fn create_endpoint(
        &self,
        api_key: &SecretString,
        request: CreateEndpointRequest,
    ) -> Result<String, RunpodError> {
        self.create_resource(api_key, "endpoints", &request).await
    }

    pub async fn delete_endpoint(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodError> {
        self.delete_resource(api_key, "endpoints", id).await
    }

    async fn graphql<T>(
        &self,
        api_key: &SecretString,
        query: &'static str,
    ) -> Result<T, RunpodError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .post(GRAPHQL_URL)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphqlRequest { query })
            .send()
            .await
            .map_err(transport_error)?;

        if let Some(error) = status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphqlResponse<T>>()
            .await
            .map_err(|_| RunpodError::InvalidResponse)?;

        graphql_data(response)
    }

    async fn create_resource<B>(
        &self,
        api_key: &SecretString,
        collection: &str,
        body: &B,
    ) -> Result<String, RunpodError>
    where
        B: Serialize + ?Sized,
    {
        let response = self
            .http
            .post(collection_url(collection)?)
            .bearer_auth(api_key.expose_secret())
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;

        if let Some(error) = status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<ResourceResponse>()
            .await
            .map_err(|_| RunpodError::InvalidResponse)?;

        map_resource_response(response)
    }

    async fn delete_resource(
        &self,
        api_key: &SecretString,
        collection: &str,
        id: &str,
    ) -> Result<(), RunpodError> {
        let response = self
            .http
            .delete(resource_url(collection, id)?)
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(transport_error)?;

        if let Some(error) = status_error(response.status()) {
            Err(error)
        } else {
            Ok(())
        }
    }
}

#[derive(Serialize)]
struct GraphqlRequest {
    query: &'static str,
}

#[derive(Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct ResourceResponse {
    id: String,
}

#[derive(Deserialize)]
struct IdentityData {
    myself: Option<IdentityUser>,
}

#[derive(Deserialize)]
struct IdentityUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct PlacementData {
    #[serde(rename = "gpuTypes")]
    gpu_types: Vec<GpuTypeData>,
    myself: Option<PlacementUser>,
}

#[derive(Deserialize)]
struct GpuTypeData {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "memoryInGb")]
    memory_gb: u64,
}

#[derive(Deserialize)]
struct PlacementUser {
    datacenters: Vec<DatacenterData>,
}

#[derive(Deserialize)]
struct DatacenterData {
    id: String,
    name: String,
    #[serde(rename = "gpuAvailability")]
    gpu_availability: Vec<GpuAvailabilityData>,
}

#[derive(Deserialize)]
struct GpuAvailabilityData {
    #[serde(rename = "gpuTypeId")]
    gpu_type_id: String,
    available: Option<bool>,
    #[serde(rename = "stockStatus")]
    stock_status: Option<String>,
}

fn graphql_data<T>(response: GraphqlResponse<T>) -> Result<T, RunpodError> {
    if !response.errors.is_empty() {
        return Err(RunpodError::RequestFailed);
    }

    response.data.ok_or(RunpodError::InvalidResponse)
}

fn map_identity(data: IdentityData) -> Result<RunpodIdentity, RunpodError> {
    let identity = data.myself.ok_or(RunpodError::InvalidResponse)?;

    Ok(RunpodIdentity {
        user_id: required(identity.id)?,
        email: required(identity.email)?,
    })
}

fn map_placement(data: PlacementData) -> Result<RunpodPlacementOptions, RunpodError> {
    let user = data.myself.ok_or(RunpodError::InvalidResponse)?;
    let gpu_types = data
        .gpu_types
        .into_iter()
        .map(|gpu| {
            Ok(RunpodGpuType {
                id: required(gpu.id)?,
                display_name: required(gpu.display_name)?,
                memory_gb: gpu.memory_gb,
            })
        })
        .collect::<Result<Vec<_>, RunpodError>>()?;
    let datacenters = user
        .datacenters
        .into_iter()
        .map(|datacenter| {
            let gpu_availability = datacenter
                .gpu_availability
                .into_iter()
                .map(|availability| {
                    Ok(RunpodGpuAvailability {
                        gpu_type_id: required(availability.gpu_type_id)?,
                        available: availability.available,
                        stock_status: normalized(availability.stock_status),
                    })
                })
                .collect::<Result<Vec<_>, RunpodError>>()?;

            Ok(RunpodDatacenter {
                id: required(datacenter.id)?,
                name: required(datacenter.name)?,
                gpu_availability,
            })
        })
        .collect::<Result<Vec<_>, RunpodError>>()?;

    Ok(RunpodPlacementOptions {
        gpu_types,
        datacenters,
    })
}

fn required(value: String) -> Result<String, RunpodError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(RunpodError::InvalidResponse)
    } else {
        Ok(value)
    }
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn collection_url(collection: &str) -> Result<Url, RunpodError> {
    let mut url = Url::parse(REST_BASE_URL).map_err(|_| RunpodError::RequestFailed)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| RunpodError::RequestFailed)?;
        segments.push(collection);
    }
    Ok(url)
}

fn resource_url(collection: &str, id: &str) -> Result<Url, RunpodError> {
    let mut url = collection_url(collection)?;
    url.path_segments_mut()
        .map_err(|_| RunpodError::RequestFailed)?
        .push(id);
    Ok(url)
}

fn map_resource_response(response: ResourceResponse) -> Result<String, RunpodError> {
    required(response.id)
}

fn transport_error(error: reqwest::Error) -> RunpodError {
    if error.is_timeout() {
        RunpodError::Timeout
    } else {
        RunpodError::RequestFailed
    }
}

fn status_error(status: StatusCode) -> Option<RunpodError> {
    if status.is_success() {
        return None;
    }

    Some(match status {
        StatusCode::UNAUTHORIZED => RunpodError::Unauthorized,
        StatusCode::FORBIDDEN => RunpodError::InsufficientPermissions,
        StatusCode::TOO_MANY_REQUESTS => RunpodError::RateLimited,
        _ => RunpodError::RequestFailed,
    })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_a_valid_identity() {
        assert_eq!(
            map_identity(IdentityData {
                myself: Some(IdentityUser {
                    id: " user-id ".to_string(),
                    email: " user@example.com ".to_string(),
                }),
            }),
            Ok(RunpodIdentity {
                user_id: "user-id".to_string(),
                email: "user@example.com".to_string(),
            })
        );
    }

    #[test]
    fn rejects_graphql_errors_and_missing_identity() {
        assert_eq!(
            graphql_data(GraphqlResponse::<IdentityData> {
                data: None,
                errors: vec![json!({})],
            })
            .map(|_| ()),
            Err(RunpodError::RequestFailed)
        );
        assert_eq!(
            map_identity(IdentityData { myself: None }),
            Err(RunpodError::InvalidResponse)
        );
    }

    #[test]
    fn preserves_placement_availability() {
        assert_eq!(
            map_placement(PlacementData {
                gpu_types: vec![GpuTypeData {
                    id: "gpu-1".to_string(),
                    display_name: "GPU One".to_string(),
                    memory_gb: 24,
                }],
                myself: Some(PlacementUser {
                    datacenters: vec![DatacenterData {
                        id: "dc-1".to_string(),
                        name: "Datacenter One".to_string(),
                        gpu_availability: vec![GpuAvailabilityData {
                            gpu_type_id: "gpu-1".to_string(),
                            available: Some(false),
                            stock_status: Some("LOW".to_string()),
                        }],
                    }],
                }),
            }),
            Ok(RunpodPlacementOptions {
                gpu_types: vec![RunpodGpuType {
                    id: "gpu-1".to_string(),
                    display_name: "GPU One".to_string(),
                    memory_gb: 24,
                }],
                datacenters: vec![RunpodDatacenter {
                    id: "dc-1".to_string(),
                    name: "Datacenter One".to_string(),
                    gpu_availability: vec![RunpodGpuAvailability {
                        gpu_type_id: "gpu-1".to_string(),
                        available: Some(false),
                        stock_status: Some("LOW".to_string()),
                    }],
                }],
            })
        );
    }

    #[test]
    fn classifies_http_statuses() {
        assert_eq!(status_error(StatusCode::OK), None);
        assert_eq!(
            status_error(StatusCode::UNAUTHORIZED),
            Some(RunpodError::Unauthorized)
        );
        assert_eq!(
            status_error(StatusCode::FORBIDDEN),
            Some(RunpodError::InsufficientPermissions)
        );
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS),
            Some(RunpodError::RateLimited)
        );
        assert_eq!(
            status_error(StatusCode::BAD_GATEWAY),
            Some(RunpodError::RequestFailed)
        );
    }

    #[test]
    fn serializes_provider_native_create_requests() {
        assert_eq!(
            serde_json::to_value(CreateNetworkVolumeRequest {
                datacenter_id: "dc-1".to_string(),
                name: "volume-name".to_string(),
                size_gb: 50,
            })
            .expect("network volume request json"),
            json!({
                "dataCenterId": "dc-1",
                "name": "volume-name",
                "size": 50
            })
        );

        assert_eq!(
            serde_json::to_value(CreatePodRequest {
                datacenter_ids: vec!["dc-1".to_string()],
                compute_type: RunpodComputeType::Cpu,
                gpu_type_ids: Vec::new(),
                image_name: "image:tag".to_string(),
                network_volume_id: "volume-1".to_string(),
                name: "pod-name".to_string(),
                ports: vec!["8000/http".to_string()],
                env: std::collections::HashMap::from([("MODE".to_string(), "test".to_string(),)]),
            })
            .expect("pod request json"),
            json!({
                "dataCenterIds": ["dc-1"],
                "computeType": "CPU",
                "gpuTypeIds": [],
                "imageName": "image:tag",
                "networkVolumeId": "volume-1",
                "name": "pod-name",
                "ports": ["8000/http"],
                "env": { "MODE": "test" }
            })
        );

        assert_eq!(
            serde_json::to_value(CreateTemplateRequest {
                image_name: "image:tag".to_string(),
                name: "template-name".to_string(),
                is_serverless: true,
            })
            .expect("template request json"),
            json!({
                "imageName": "image:tag",
                "name": "template-name",
                "isServerless": true
            })
        );

        assert_eq!(
            serde_json::to_value(CreateEndpointRequest {
                datacenter_ids: vec!["dc-1".to_string()],
                gpu_type_ids: vec!["gpu-1".to_string()],
                name: "endpoint-name".to_string(),
                network_volume_id: "volume-1".to_string(),
                template_id: "template-1".to_string(),
                workers_min: 0,
                workers_max: 1,
            })
            .expect("endpoint request json"),
            json!({
                "dataCenterIds": ["dc-1"],
                "gpuTypeIds": ["gpu-1"],
                "name": "endpoint-name",
                "networkVolumeId": "volume-1",
                "templateId": "template-1",
                "workersMin": 0,
                "workersMax": 1
            })
        );
    }

    #[test]
    fn encodes_resource_ids_as_one_path_segment() {
        assert_eq!(
            resource_url("pods", "pod/a?b")
                .expect("resource url")
                .as_str(),
            "https://rest.runpod.io/v1/pods/pod%2Fa%3Fb"
        );
    }

    #[test]
    fn validates_created_resource_ids() {
        assert_eq!(
            map_resource_response(ResourceResponse {
                id: " resource-1 ".to_string(),
            }),
            Ok("resource-1".to_string())
        );
        assert_eq!(
            map_resource_response(ResourceResponse {
                id: " ".to_string(),
            }),
            Err(RunpodError::InvalidResponse)
        );
    }
}
