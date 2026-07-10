use reqwest::header::CONTENT_TYPE;
use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use crate::infra::providers::{
    graphql::{Gql, GqlResponseExt},
    http,
    http::ResponseExt,
    ProviderError,
};

use super::types::{
    RunpodDatacenter, RunpodGpuAvailability, RunpodGpuType, RunpodIdentity, RunpodPlacementOptions,
};

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const IDENTITY_QUERY: &str = include_str!("queries/identity.graphql");
const PLACEMENT_QUERY: &str = include_str!("queries/placement.graphql");

#[derive(Clone)]
pub struct RunpodClient {
    http: reqwest::Client,
}

impl RunpodClient {
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            http: http::client()?,
        })
    }

    pub async fn identity(&self, api_key: &SecretString) -> Result<RunpodIdentity, ProviderError> {
        self.get_identity(api_key).await
    }

    pub async fn placement_options(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacementOptions, ProviderError> {
        map_placement(
            self.graphql::<PlacementOptionsResponse>(api_key, PLACEMENT_QUERY)
                .await?,
        )
    }

    pub async fn create_network_volume(
        &self,
        api_key: &SecretString,
        request: CreateNetworkVolumeRequest,
    ) -> Result<String, ProviderError> {
        let response = self
            .create_resource::<_, CreateNetworkVolumeResponse>(api_key, "networkvolumes", &request)
            .await?;
        Ok(response.id)
    }

    pub async fn delete_network_volume(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "networkvolumes", id).await
    }

    pub async fn create_pod(
        &self,
        api_key: &SecretString,
        request: CreatePodRequest,
    ) -> Result<String, ProviderError> {
        let response = self
            .create_resource::<_, CreatePodResponse>(api_key, "pods", &request)
            .await?;
        Ok(response.id)
    }

    pub async fn create_endpoint(
        &self,
        api_key: &SecretString,
        request: CreateEndpointRequest,
    ) -> Result<String, ProviderError> {
        let response = self
            .create_resource::<_, CreateEndpointResponse>(api_key, "endpoints", &request)
            .await?;
        Ok(response.id)
    }

    pub async fn create_template(
        &self,
        api_key: &SecretString,
        request: CreateTemplateRequest,
    ) -> Result<String, ProviderError> {
        let response = self
            .create_resource::<_, CreateTemplateResponse>(api_key, "templates", &request)
            .await?;
        Ok(response.id)
    }

    pub async fn delete_pod(&self, api_key: &SecretString, id: &str) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "pods", id).await
    }

    pub async fn delete_template(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "templates", id).await
    }

    pub async fn delete_endpoint(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "endpoints", id).await
    }

    async fn get_placement_options(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacementOptions, ProviderError> {
        map_placement(self.graphql::<PlacementOptionsResponse>(api_key, PLACEMENT_QUERY).await?)
    }

    async fn get_identity(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodIdentity, ProviderError> {
        map_identity(self.graphql::<IdentityResponse>(api_key, IDENTITY_QUERY).await?)
    }

    async fn graphql<T>(
        &self,
        api_key: &SecretString,
        query: &'static str,
    ) -> Result<T, ProviderError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .post(GRAPHQL_URL)
            .bearer_auth(api_key.expose_secret())
            .header(CONTENT_TYPE, "application/json")
            .body(Gql::new(query).build(json!({})))
            .send()
            .await;

        response.provider_gql_json::<T>().await
    }

    async fn create_resource<B, R>(
        &self,
        api_key: &SecretString,
        collection: &str,
        body: &B,
    ) -> Result<R, ProviderError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.http
            .post(format!("{REST_BASE_URL}/{collection}"))
            .bearer_auth(api_key.expose_secret())
            .json(body)
            .send()
            .await
            .provider_json::<R>()
            .await
    }

    async fn delete_resource(
        &self,
        api_key: &SecretString,
        collection: &str,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.http
            .delete(format!("{REST_BASE_URL}/{collection}/{id}"))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .provider_response()?;
        Ok(())
    }
}

#[derive(Serialize)]
pub struct CreateNetworkVolumeRequest {
    #[serde(rename = "dataCenterId")]
    pub datacenter_id: String,
    pub name: String,
    #[serde(rename = "size")]
    pub size_gb: u64,
}

#[derive(Deserialize)]
struct CreateNetworkVolumeResponse {
    id: String,
}

#[derive(Clone, Copy, Serialize)]
pub enum CreatePodComputeType {
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
    pub compute_type: CreatePodComputeType,
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

#[derive(Deserialize)]
struct CreatePodResponse {
    id: String,
}

#[derive(Serialize)]
pub struct CreateTemplateRequest {
    #[serde(rename = "imageName")]
    pub image_name: String,
    pub name: String,
    #[serde(rename = "isServerless")]
    pub is_serverless: bool,
}

#[derive(Deserialize)]
struct CreateTemplateResponse {
    id: String,
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

#[derive(Deserialize)]
struct CreateEndpointResponse {
    id: String,
}

#[derive(Deserialize)]
struct IdentityResponse {
    myself: Option<IdentityUserResponse>,
}

#[derive(Deserialize)]
struct IdentityUserResponse {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct PlacementOptionsResponse {
    #[serde(rename = "gpuTypes")]
    gpu_types: Vec<PlacementOptionsGpuTypeResponse>,
    myself: Option<PlacementOptionsUserResponse>,
}

#[derive(Deserialize)]
struct PlacementOptionsGpuTypeResponse {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "memoryInGb")]
    memory_gb: u64,
}

#[derive(Deserialize)]
struct PlacementOptionsUserResponse {
    datacenters: Vec<PlacementOptionsDatacenterResponse>,
}

#[derive(Deserialize)]
struct PlacementOptionsDatacenterResponse {
    id: String,
    name: String,
    #[serde(rename = "gpuAvailability")]
    gpu_availability: Vec<PlacementOptionsGpuAvailabilityResponse>,
}

#[derive(Deserialize)]
struct PlacementOptionsGpuAvailabilityResponse {
    #[serde(rename = "gpuTypeId")]
    gpu_type_id: String,
    available: Option<bool>,
    #[serde(rename = "stockStatus")]
    stock_status: Option<String>,
}

fn map_identity(data: IdentityResponse) -> Result<RunpodIdentity, ProviderError> {
    let identity = data.myself.ok_or(ProviderError::InvalidResponse)?;

    Ok(RunpodIdentity {
        user_id: identity.id,
        email: identity.email,
    })
}

fn map_placement(data: PlacementOptionsResponse) -> Result<RunpodPlacementOptions, ProviderError> {
    let user = data.myself.ok_or(ProviderError::InvalidResponse)?;
    let gpu_types = data
        .gpu_types
        .into_iter()
        .map(|gpu| RunpodGpuType {
            id: gpu.id,
            display_name: gpu.display_name,
            memory_gb: gpu.memory_gb,
        })
        .collect();
    let datacenters = user
        .datacenters
        .into_iter()
        .map(|datacenter| {
            let gpu_availability = datacenter
                .gpu_availability
                .into_iter()
                .map(|availability| RunpodGpuAvailability {
                    gpu_type_id: availability.gpu_type_id,
                    available: availability.available,
                    stock_status: availability.stock_status,
                })
                .collect();

            RunpodDatacenter {
                id: datacenter.id,
                name: datacenter.name,
                gpu_availability,
            }
        })
        .collect();

    Ok(RunpodPlacementOptions {
        gpu_types,
        datacenters,
    })
}
