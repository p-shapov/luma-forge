use std::time::Duration;

mod contracts;
mod mapper;

use crate::{
    domain::{
        provider_inventory::ProviderInventory, provider_setup::ProviderApiKey,
        workspace::ProviderResourceStatus,
    },
    provider::{
        error::ProviderClientError,
        runpod::contracts::{
            GraphQlRequest, GraphQlResponse, RunPodEndpointResponse, RunPodIdentityData,
            RunPodInventoryData, RunPodNetworkVolumeResponse, RunPodPodResponse,
            RunPodTemplateResponse,
        },
    },
};

pub use contracts::{
    RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest, RunPodCreatePodRequest,
    RunPodCreateTemplateRequest, RunPodEndpointObservation, RunPodNetworkVolumeObservation,
    RunPodPodObservation, RunPodTemplateObservation,
};

use mapper::{
    endpoint_from_response, identity_from_graphql_response, inventory_from_graphql_response,
    network_volume_from_response, pod_from_response, template_from_response,
};

const RUNPOD_GRAPHQL_ENDPOINT: &str = "https://api.runpod.io/graphql";
const RUNPOD_REST_ENDPOINT: &str = "https://rest.runpod.io/v1";
const RUNPOD_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RUNPOD_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

const IDENTITY_QUERY: &str = r#"
query LumaForgeProviderIdentity {
  myself {
    email
    apiKeys {
      id
      isActive
    }
  }
}
"#;
const INVENTORY_QUERY: &str = r#"
query LumaForgeProviderInventory {
  dataCenters {
    id
    name
    storageSupport
    gpuAvailability {
      stockStatus
      gpuType {
        id
        displayName
        memoryInGb
      }
    }
  }
}
"#;

#[derive(Debug, Clone)]
pub struct RunPodClient {
    http: reqwest::Client,
    graphql_endpoint: String,
    rest_endpoint: String,
}

impl Default for RunPodClient {
    fn default() -> Self {
        Self::new(
            RUNPOD_GRAPHQL_ENDPOINT.to_string(),
            RUNPOD_CONNECT_TIMEOUT,
            RUNPOD_REQUEST_TIMEOUT,
        )
    }
}

impl RunPodClient {
    fn new(endpoint: String, connect_timeout: Duration, request_timeout: Duration) -> Self {
        Self::new_with_endpoints(
            endpoint,
            default_rest_endpoint(),
            connect_timeout,
            request_timeout,
        )
    }

    fn new_with_endpoints(
        graphql_endpoint: String,
        rest_endpoint: String,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .connect_timeout(connect_timeout)
                .timeout(request_timeout)
                .build()
                .expect("RunPod HTTP client should build"),
            graphql_endpoint,
            rest_endpoint,
        }
    }

    pub async fn validate_identity(
        &self,
        api_key: &ProviderApiKey,
    ) -> Result<crate::domain::provider_setup::ProviderIdentity, ProviderClientError> {
        let response = self
            .http
            .post(&self.graphql_endpoint)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphQlRequest {
                query: IDENTITY_QUERY,
            })
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderClientError::Unauthorized);
        }
        if !status.is_success() {
            return Err(ProviderClientError::ApiUnavailable);
        }

        let payload = response
            .json::<GraphQlResponse<RunPodIdentityData>>()
            .await
            .map_err(|_| ProviderClientError::ResponseInvalid)?;

        identity_from_graphql_response(api_key, payload)
    }

    pub async fn fetch_inventory(
        &self,
        api_key: &ProviderApiKey,
    ) -> Result<ProviderInventory, ProviderClientError> {
        let response = self
            .http
            .post(&self.graphql_endpoint)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphQlRequest {
                query: INVENTORY_QUERY,
            })
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        if let Some(error) = provider_error_from_inventory_status(response.status()) {
            return Err(error);
        }

        let payload = response
            .json::<GraphQlResponse<RunPodInventoryData>>()
            .await
            .map_err(|_| ProviderClientError::ResponseInvalid)?;

        inventory_from_graphql_response(payload)
    }

    pub async fn create_network_volume(
        &self,
        api_key: &ProviderApiKey,
        request: &RunPodCreateNetworkVolumeRequest,
    ) -> Result<RunPodNetworkVolumeObservation, ProviderClientError> {
        let response = self
            .http
            .post(format!("{}/networkvolumes", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|_| ProviderClientError::Indeterminate)?;
        parse_rest_response::<RunPodNetworkVolumeResponse>(response)
            .await
            .and_then(network_volume_from_response)
    }

    pub async fn get_network_volume(
        &self,
        api_key: &ProviderApiKey,
        volume_id: &str,
    ) -> Result<RunPodNetworkVolumeObservation, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/networkvolumes/{volume_id}", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        parse_rest_response::<RunPodNetworkVolumeResponse>(response)
            .await
            .and_then(network_volume_from_response)
    }

    pub async fn find_network_volumes_by_name(
        &self,
        api_key: &ProviderApiKey,
        name: &str,
    ) -> Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/networkvolumes", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodNetworkVolumeResponse>>(response).await?;
        network_volumes_by_name(payloads, name)
    }

    pub async fn delete_network_volume(
        &self,
        api_key: &ProviderApiKey,
        volume_id: &str,
    ) -> Result<(), ProviderClientError> {
        self.delete_rest_resource(api_key, &format!("networkvolumes/{volume_id}"))
            .await
    }

    pub async fn create_pod(
        &self,
        api_key: &ProviderApiKey,
        request: &RunPodCreatePodRequest,
    ) -> Result<RunPodPodObservation, ProviderClientError> {
        let response = self
            .http
            .post(format!("{}/pods", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|_| ProviderClientError::Indeterminate)?;
        parse_rest_response::<RunPodPodResponse>(response)
            .await
            .and_then(pod_from_response)
    }

    pub async fn get_pod(
        &self,
        api_key: &ProviderApiKey,
        pod_id: &str,
    ) -> Result<RunPodPodObservation, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/pods/{pod_id}", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        parse_rest_response::<RunPodPodResponse>(response)
            .await
            .and_then(pod_from_response)
    }

    pub async fn find_pods_by_name(
        &self,
        api_key: &ProviderApiKey,
        name: &str,
    ) -> Result<Vec<RunPodPodObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/pods", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodPodResponse>>(response).await?;
        pods_by_name(payloads, name)
    }

    pub async fn delete_pod(
        &self,
        api_key: &ProviderApiKey,
        pod_id: &str,
    ) -> Result<(), ProviderClientError> {
        self.delete_rest_resource(api_key, &format!("pods/{pod_id}"))
            .await
    }

    pub async fn create_template(
        &self,
        api_key: &ProviderApiKey,
        request: &RunPodCreateTemplateRequest,
    ) -> Result<RunPodTemplateObservation, ProviderClientError> {
        let response = self
            .http
            .post(format!("{}/templates", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|_| ProviderClientError::Indeterminate)?;
        parse_rest_response::<RunPodTemplateResponse>(response)
            .await
            .and_then(template_from_response)
    }

    pub async fn get_template(
        &self,
        api_key: &ProviderApiKey,
        template_id: &str,
    ) -> Result<RunPodTemplateObservation, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/templates/{template_id}", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        parse_rest_response::<RunPodTemplateResponse>(response)
            .await
            .and_then(template_from_response)
    }

    pub async fn find_templates_by_name(
        &self,
        api_key: &ProviderApiKey,
        name: &str,
    ) -> Result<Vec<RunPodTemplateObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/templates", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodTemplateResponse>>(response).await?;
        templates_by_name(payloads, name)
    }

    pub async fn delete_template(
        &self,
        api_key: &ProviderApiKey,
        template_id: &str,
    ) -> Result<(), ProviderClientError> {
        self.delete_rest_resource(api_key, &format!("templates/{template_id}"))
            .await
    }

    pub async fn create_endpoint(
        &self,
        api_key: &ProviderApiKey,
        request: &RunPodCreateEndpointRequest,
    ) -> Result<RunPodEndpointObservation, ProviderClientError> {
        let response = self
            .http
            .post(format!("{}/endpoints", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .json(request)
            .send()
            .await
            .map_err(|_| ProviderClientError::Indeterminate)?;
        parse_rest_response::<RunPodEndpointResponse>(response)
            .await
            .and_then(endpoint_from_response)
    }

    pub async fn get_endpoint(
        &self,
        api_key: &ProviderApiKey,
        endpoint_id: &str,
    ) -> Result<RunPodEndpointObservation, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/endpoints/{endpoint_id}", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        parse_rest_response::<RunPodEndpointResponse>(response)
            .await
            .and_then(endpoint_from_response)
    }

    pub async fn find_endpoints_by_name(
        &self,
        api_key: &ProviderApiKey,
        name: &str,
    ) -> Result<Vec<RunPodEndpointObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/endpoints", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodEndpointResponse>>(response).await?;
        endpoints_by_name(payloads, name)
    }

    pub async fn delete_endpoint(
        &self,
        api_key: &ProviderApiKey,
        endpoint_id: &str,
    ) -> Result<(), ProviderClientError> {
        self.delete_rest_resource(api_key, &format!("endpoints/{endpoint_id}"))
            .await
    }

    async fn delete_rest_resource(
        &self,
        api_key: &ProviderApiKey,
        path: &str,
    ) -> Result<(), ProviderClientError> {
        let response = self
            .http
            .delete(format!("{}/{}", self.rest_endpoint, path))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        provider_error_from_rest_status(response.status()).map_or(Ok(()), Err)
    }
}

fn default_rest_endpoint() -> String {
    RUNPOD_REST_ENDPOINT.to_string()
}

fn network_volumes_by_name(
    payloads: Vec<RunPodNetworkVolumeResponse>,
    name: &str,
) -> Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| payload.name.as_deref() == Some(name))
        .map(network_volume_from_discovery_response)
        .collect()
}

fn pods_by_name(
    payloads: Vec<RunPodPodResponse>,
    name: &str,
) -> Result<Vec<RunPodPodObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| payload.name.as_deref() == Some(name))
        .filter(|payload| !pod_response_is_deleted(payload))
        .map(pod_from_discovery_response)
        .collect()
}

fn templates_by_name(
    payloads: Vec<RunPodTemplateResponse>,
    name: &str,
) -> Result<Vec<RunPodTemplateObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| payload.name.as_deref() == Some(name))
        .map(template_from_discovery_response)
        .collect()
}

fn endpoints_by_name(
    payloads: Vec<RunPodEndpointResponse>,
    name: &str,
) -> Result<Vec<RunPodEndpointObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| payload.name.as_deref() == Some(name))
        .map(endpoint_from_discovery_response)
        .collect()
}

fn network_volume_from_discovery_response(
    payload: RunPodNetworkVolumeResponse,
) -> Result<RunPodNetworkVolumeObservation, ProviderClientError> {
    Ok(RunPodNetworkVolumeObservation {
        id: required_non_empty(payload.id)?,
        status: ProviderResourceStatus::Unknown,
    })
}

fn pod_from_discovery_response(
    payload: RunPodPodResponse,
) -> Result<RunPodPodObservation, ProviderClientError> {
    let id = required_non_empty(payload.id)?;
    Ok(RunPodPodObservation {
        provisioner_status_url: None,
        id,
        status: ProviderResourceStatus::Unknown,
    })
}

fn template_from_discovery_response(
    payload: RunPodTemplateResponse,
) -> Result<RunPodTemplateObservation, ProviderClientError> {
    Ok(RunPodTemplateObservation {
        id: required_non_empty(payload.id)?,
        image_name: payload.image_name.unwrap_or_default(),
        volume_mount_path: payload.volume_mount_path.unwrap_or_default(),
        status: ProviderResourceStatus::Unknown,
    })
}

fn endpoint_from_discovery_response(
    payload: RunPodEndpointResponse,
) -> Result<RunPodEndpointObservation, ProviderClientError> {
    let id = required_non_empty(payload.id)?;
    Ok(RunPodEndpointObservation {
        endpoint_invoke_url: payload
            .endpoint_url
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("https://api.runpod.ai/v2/{id}/run")),
        id,
        status: ProviderResourceStatus::Unknown,
    })
}

fn required_non_empty(value: Option<String>) -> Result<String, ProviderClientError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProviderClientError::ResponseInvalid)
}

fn pod_response_is_deleted(payload: &RunPodPodResponse) -> bool {
    matches!(
        payload
            .pod_status
            .as_deref()
            .or(payload.desired_status.as_deref())
            .unwrap_or_default()
            .to_ascii_uppercase()
            .as_str(),
        "EXITED" | "STOPPED" | "TERMINATED" | "DELETED"
    )
}

pub(super) fn provider_error_from_inventory_status(
    status: reqwest::StatusCode,
) -> Option<ProviderClientError> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        Some(ProviderClientError::Unauthorized)
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        Some(ProviderClientError::RateLimited)
    } else if status.is_client_error() {
        Some(ProviderClientError::RequestRejected)
    } else if !status.is_success() {
        Some(ProviderClientError::ApiUnavailable)
    } else {
        None
    }
}

async fn parse_rest_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ProviderClientError> {
    if let Some(error) = provider_error_from_rest_status(response.status()) {
        return Err(error);
    }
    response
        .json::<T>()
        .await
        .map_err(|_| ProviderClientError::ResponseInvalid)
}

pub(super) fn provider_error_from_rest_status(
    status: reqwest::StatusCode,
) -> Option<ProviderClientError> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        Some(ProviderClientError::Unauthorized)
    } else if status == reqwest::StatusCode::NOT_FOUND {
        Some(ProviderClientError::NotFound)
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        Some(ProviderClientError::RateLimited)
    } else if status == reqwest::StatusCode::CONFLICT {
        Some(ProviderClientError::Conflict)
    } else if status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::GATEWAY_TIMEOUT
    {
        Some(ProviderClientError::Indeterminate)
    } else if status.is_client_error() {
        Some(ProviderClientError::RequestRejected)
    } else if !status.is_success() {
        Some(ProviderClientError::ApiUnavailable)
    } else {
        None
    }
}
