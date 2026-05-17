use std::{collections::HashMap, time::Duration};

mod contracts;
mod mapper;

use crate::{
    domain::{provider_inventory::ProviderInventory, provider_setup::ProviderApiKey},
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
    network_volume_from_list_response, network_volume_from_response,
    pod_from_response_with_context, template_from_response, RunPodPodResponseContext,
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
            RUNPOD_REST_ENDPOINT.to_string(),
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

    #[cfg(test)]
    pub(super) fn new_for_test(endpoint: String, request_timeout: Duration) -> Self {
        Self::new_with_endpoints(endpoint.clone(), endpoint, request_timeout, request_timeout)
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
        data_center_id: &str,
        size_gb: u64,
    ) -> Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/networkvolumes", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodNetworkVolumeResponse>>(response).await?;
        network_volumes_by_name(payloads, name, data_center_id, size_gb)
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
        let context = RunPodPodResponseContext {
            data_center_id: request
                .data_center_ids
                .first()
                .cloned()
                .ok_or(ProviderClientError::ResponseInvalid)?,
            selected_gpu_id: request
                .gpu_type_ids
                .first()
                .cloned()
                .ok_or(ProviderClientError::ResponseInvalid)?,
        };
        parse_rest_response::<RunPodPodResponse>(response)
            .await
            .and_then(|payload| pod_from_response_with_context(payload, Some(context)))
    }

    pub async fn get_pod_with_context(
        &self,
        api_key: &ProviderApiKey,
        pod_id: &str,
        data_center_id: &str,
        selected_gpu_id: &str,
    ) -> Result<RunPodPodObservation, ProviderClientError> {
        self.get_pod_mapped(
            api_key,
            pod_id,
            Some(RunPodPodResponseContext {
                data_center_id: data_center_id.to_string(),
                selected_gpu_id: selected_gpu_id.to_string(),
            }),
        )
        .await
    }

    async fn get_pod_mapped(
        &self,
        api_key: &ProviderApiKey,
        pod_id: &str,
        context: Option<RunPodPodResponseContext>,
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
            .and_then(|payload| pod_from_response_with_context(payload, context))
    }

    pub async fn find_pods_by_name_and_volume(
        &self,
        api_key: &ProviderApiKey,
        name: &str,
        network_volume_id: &str,
        data_center_id: &str,
        selected_gpu_id: &str,
    ) -> Result<Vec<RunPodPodObservation>, ProviderClientError> {
        let context = RunPodPodResponseContext {
            data_center_id: data_center_id.to_string(),
            selected_gpu_id: selected_gpu_id.to_string(),
        };
        let response = self
            .http
            .get(format!("{}/pods", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodPodResponse>>(response).await?;
        pods_by_name_and_volume(payloads, name, network_volume_id, context)
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
        image_name: &str,
        expected_env: &HashMap<String, String>,
        http_port: u16,
        volume_mount_path: &str,
    ) -> Result<Vec<RunPodTemplateObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/templates", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodTemplateResponse>>(response).await?;
        templates_by_name(
            payloads,
            name,
            image_name,
            expected_env,
            http_port,
            volume_mount_path,
        )
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
        input: &RunPodFindEndpointInput<'_>,
    ) -> Result<Vec<RunPodEndpointObservation>, ProviderClientError> {
        let response = self
            .http
            .get(format!("{}/endpoints", self.rest_endpoint))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;
        let payloads = parse_rest_response::<Vec<RunPodEndpointResponse>>(response).await?;
        endpoints_by_name(payloads, input)
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

pub struct RunPodFindEndpointInput<'a> {
    pub name: &'a str,
    pub template_id: &'a str,
    pub network_volume_id: &'a str,
    pub data_center_id: &'a str,
    pub selected_gpu_id: &'a str,
    pub idle_timeout: u32,
}

fn network_volumes_by_name(
    payloads: Vec<RunPodNetworkVolumeResponse>,
    name: &str,
    data_center_id: &str,
    size_gb: u64,
) -> Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| {
            payload.name.as_deref() == Some(name)
                && payload.data_center_id.as_deref() == Some(data_center_id)
                && payload.size == Some(size_gb)
        })
        .map(network_volume_from_list_response)
        .collect()
}

fn pods_by_name_and_volume(
    payloads: Vec<RunPodPodResponse>,
    name: &str,
    network_volume_id: &str,
    context: RunPodPodResponseContext,
) -> Result<Vec<RunPodPodObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| {
            payload.name.as_deref() == Some(name)
                && payload.network_volume_id.as_deref() == Some(network_volume_id)
        })
        .filter(|payload| !pod_response_is_deleted(payload))
        .map(|payload| pod_from_response_with_context(payload, Some(context.clone())))
        .collect()
}

fn templates_by_name(
    payloads: Vec<RunPodTemplateResponse>,
    name: &str,
    image_name: &str,
    expected_env: &HashMap<String, String>,
    http_port: u16,
    volume_mount_path: &str,
) -> Result<Vec<RunPodTemplateObservation>, ProviderClientError> {
    let expected_port = format!("{http_port}/http");
    payloads
        .into_iter()
        .filter(|payload| {
            payload.name.as_deref() == Some(name)
                && payload.image_name.as_deref() == Some(image_name)
                && (expected_env.is_empty()
                    || payload.env.as_ref().is_some_and(|env| {
                        expected_env
                            .iter()
                            .all(|(key, value)| env.get(key) == Some(value))
                    }))
                && payload.volume_mount_path.as_deref() == Some(volume_mount_path)
                && payload.is_serverless == Some(true)
                && payload
                    .ports
                    .as_ref()
                    .is_some_and(|ports| ports.iter().any(|port| port == &expected_port))
        })
        .map(template_from_response)
        .collect()
}

fn endpoints_by_name(
    payloads: Vec<RunPodEndpointResponse>,
    input: &RunPodFindEndpointInput<'_>,
) -> Result<Vec<RunPodEndpointObservation>, ProviderClientError> {
    payloads
        .into_iter()
        .filter(|payload| {
            payload.name.as_deref() == Some(input.name)
                && payload.template_id.as_deref() == Some(input.template_id)
                && payload.network_volume_id.as_deref() == Some(input.network_volume_id)
                && payload
                    .data_center_ids
                    .as_ref()
                    .is_some_and(|ids| ids.iter().any(|id| id == input.data_center_id))
                && payload
                    .gpu_type_ids
                    .as_ref()
                    .is_some_and(|ids| ids.iter().any(|id| id == input.selected_gpu_id))
                && payload
                    .idle_timeout
                    .is_none_or(|timeout| timeout == input.idle_timeout)
        })
        .map(endpoint_from_response)
        .collect()
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
