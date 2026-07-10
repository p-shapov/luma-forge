use reqwest::Url;
use secrecy::{ExposeSecret, SecretString};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::infra::providers::{http, ProviderError};

use super::types::{
    CreateEndpointRequest, CreateNetworkVolumeRequest, CreatePodRequest, CreateTemplateRequest,
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
        map_identity(self.graphql(api_key, IDENTITY_QUERY).await?)
    }

    pub async fn placement_options(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacementOptions, ProviderError> {
        map_placement(self.graphql(api_key, PLACEMENT_QUERY).await?)
    }

    pub async fn create_network_volume(
        &self,
        api_key: &SecretString,
        request: CreateNetworkVolumeRequest,
    ) -> Result<String, ProviderError> {
        self.create_resource(api_key, "networkvolumes", &request)
            .await
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
        self.create_resource(api_key, "pods", &request).await
    }

    pub async fn delete_pod(&self, api_key: &SecretString, id: &str) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "pods", id).await
    }

    pub async fn create_template(
        &self,
        api_key: &SecretString,
        request: CreateTemplateRequest,
    ) -> Result<String, ProviderError> {
        self.create_resource(api_key, "templates", &request).await
    }

    pub async fn delete_template(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "templates", id).await
    }

    pub async fn create_endpoint(
        &self,
        api_key: &SecretString,
        request: CreateEndpointRequest,
    ) -> Result<String, ProviderError> {
        self.create_resource(api_key, "endpoints", &request).await
    }

    pub async fn delete_endpoint(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), ProviderError> {
        self.delete_resource(api_key, "endpoints", id).await
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
            .json(&GraphqlRequest { query })
            .send()
            .await
            .map_err(http::transport_error)?;

        if let Some(error) = http::status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphqlResponse<T>>()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;

        graphql_data(response)
    }

    async fn create_resource<B>(
        &self,
        api_key: &SecretString,
        collection: &str,
        body: &B,
    ) -> Result<String, ProviderError>
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
            .map_err(http::transport_error)?;

        if let Some(error) = http::status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<ResourceResponse>()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;

        map_resource_response(response)
    }

    async fn delete_resource(
        &self,
        api_key: &SecretString,
        collection: &str,
        id: &str,
    ) -> Result<(), ProviderError> {
        let response = self
            .http
            .delete(resource_url(collection, id)?)
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(http::transport_error)?;

        if let Some(error) = http::status_error(response.status()) {
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

fn graphql_data<T>(response: GraphqlResponse<T>) -> Result<T, ProviderError> {
    if !response.errors.is_empty() {
        return Err(ProviderError::RequestFailed);
    }

    response.data.ok_or(ProviderError::InvalidResponse)
}

fn map_identity(data: IdentityData) -> Result<RunpodIdentity, ProviderError> {
    let identity = data.myself.ok_or(ProviderError::InvalidResponse)?;

    Ok(RunpodIdentity {
        user_id: required(identity.id)?,
        email: required(identity.email)?,
    })
}

fn map_placement(data: PlacementData) -> Result<RunpodPlacementOptions, ProviderError> {
    let user = data.myself.ok_or(ProviderError::InvalidResponse)?;
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
        .collect::<Result<Vec<_>, ProviderError>>()?;
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
                .collect::<Result<Vec<_>, ProviderError>>()?;

            Ok(RunpodDatacenter {
                id: required(datacenter.id)?,
                name: required(datacenter.name)?,
                gpu_availability,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;

    Ok(RunpodPlacementOptions {
        gpu_types,
        datacenters,
    })
}

fn required(value: String) -> Result<String, ProviderError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        Err(ProviderError::InvalidResponse)
    } else {
        Ok(value)
    }
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn collection_url(collection: &str) -> Result<Url, ProviderError> {
    let mut url = Url::parse(REST_BASE_URL).map_err(|_| ProviderError::RequestFailed)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| ProviderError::RequestFailed)?;
        segments.push(collection);
    }
    Ok(url)
}

fn resource_url(collection: &str, id: &str) -> Result<Url, ProviderError> {
    let mut url = collection_url(collection)?;
    url.path_segments_mut()
        .map_err(|_| ProviderError::RequestFailed)?
        .push(id);
    Ok(url)
}

fn map_resource_response(response: ResourceResponse) -> Result<String, ProviderError> {
    required(response.id)
}
