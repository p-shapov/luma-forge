use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{runpod::RunpodPlacementOptions, secrets::ApiKeyIdentity},
    provider::errors::ProviderApiError,
};

use super::runtime::RunpodProvisionerStatus;
use super::{
    errors::RunpodProviderError,
    mapping::{
        self, EndpointCreateBody, EndpointResponse, GraphqlResponse, NetworkVolumeCreateBody,
        NetworkVolumeResponse, PlacementQueryData, PodCreateBody, PodResponse, TemplateCreateBody,
        TemplateResponse,
    },
};
use crate::secrets::errors::{
    identity_request_error, identity_response_invalid_error, identity_status_error,
    SecretsStorageError,
};

const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
pub(super) const RUNPOD_PROVIDER_NAME: &str = "RunPod";
const PROVISIONER_PORT: u16 = 8000;
const RUNPOD_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const RUNPOD_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const RUNPOD_IDENTITY_QUERY: &str =
    "query LumaForgeRunpodIdentity { myself { email apiKeys { id isActive } } }";

const STATUS_IDLE: &str = "idle";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

#[derive(Clone)]
pub struct RunpodApiClient {
    http: reqwest::Client,
    rest_base_url: String,
    graphql_url: String,
}

impl RunpodApiClient {
    pub(super) fn new() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(RUNPOD_CONNECT_TIMEOUT)
                .timeout(RUNPOD_REQUEST_TIMEOUT)
                .build()
                .map_err(identity_request_error)?,
            rest_base_url: RUNPOD_REST_BASE_URL.to_string(),
            graphql_url: RUNPOD_GRAPHQL_URL.to_string(),
        })
    }

    pub(super) async fn get_identity(
        &self,
        api_key: String,
    ) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let response = self
            .http
            .post(&self.graphql_url)
            .bearer_auth(&api_key)
            .json(&GraphQlRequest {
                query: RUNPOD_IDENTITY_QUERY,
            })
            .send()
            .await
            .map_err(identity_request_error)?;

        if let Some(error) = identity_status_error(RUNPOD_PROVIDER_NAME, response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphQlResponse<RunpodIdentityData>>()
            .await
            .map_err(identity_response_invalid_error)?;

        super::identity::map_graphql_response(&api_key, response)
    }

    pub(super) async fn placement_options_request(
        &self,
        api_key: &str,
    ) -> Result<RunpodPlacementOptions, ProviderApiError> {
        let response = self
            .http
            .post(&self.graphql_url)
            .bearer_auth(api_key)
            .json(&mapping::placement_graphql_request())
            .send()
            .await
            .map_err(mapping::map_send_error)?;

        let response: GraphqlResponse<PlacementQueryData> =
            mapping::parse_json_response(response).await?;

        mapping::map_placement_response(response)
    }

    pub(super) async fn create_network_volume(
        &self,
        api_key: &str,
        body: &NetworkVolumeCreateBody,
    ) -> Result<NetworkVolumeResponse, ProviderApiError> {
        self.post_rest(api_key, "/networkvolumes", body).await
    }

    pub(super) async fn delete_network_volume(
        &self,
        api_key: &str,
        network_volume_id: &str,
    ) -> Result<(), ProviderApiError> {
        self.delete_rest(api_key, &format!("/networkvolumes/{network_volume_id}"))
            .await
    }

    pub(super) async fn start_pod(
        &self,
        api_key: &str,
        body: &PodCreateBody,
    ) -> Result<PodResponse, ProviderApiError> {
        self.post_rest(api_key, "/pods", body).await
    }

    pub(super) async fn delete_pod(
        &self,
        api_key: &str,
        pod_id: &str,
    ) -> Result<(), ProviderApiError> {
        self.delete_rest(api_key, &format!("/pods/{pod_id}")).await
    }

    pub(super) async fn create_template(
        &self,
        api_key: &str,
        body: &TemplateCreateBody,
    ) -> Result<TemplateResponse, ProviderApiError> {
        self.post_rest(api_key, "/templates", body).await
    }

    pub(super) async fn delete_template(
        &self,
        api_key: &str,
        template_id: &str,
    ) -> Result<(), ProviderApiError> {
        self.delete_rest(api_key, &format!("/templates/{template_id}"))
            .await
    }

    pub(super) async fn create_endpoint(
        &self,
        api_key: &str,
        body: &EndpointCreateBody,
    ) -> Result<EndpointResponse, ProviderApiError> {
        self.post_rest(api_key, "/endpoints", body).await
    }

    pub(super) async fn delete_endpoint(
        &self,
        api_key: &str,
        endpoint_id: &str,
    ) -> Result<(), ProviderApiError> {
        self.delete_rest(api_key, &format!("/endpoints/{endpoint_id}"))
            .await
    }

    async fn post_rest<B, T>(
        &self,
        api_key: &str,
        path: &str,
        body: &B,
    ) -> Result<T, ProviderApiError>
    where
        B: serde::Serialize + ?Sized,
        T: for<'de> serde::Deserialize<'de>,
    {
        let response = self
            .http
            .request(Method::POST, format!("{}{path}", self.rest_base_url))
            .bearer_auth(api_key)
            .json(body)
            .send()
            .await
            .map_err(mapping::map_send_error)?;

        mapping::parse_json_response(response).await
    }

    async fn delete_rest(&self, api_key: &str, path: &str) -> Result<(), ProviderApiError> {
        let response = self
            .http
            .request(Method::DELETE, format!("{}{path}", self.rest_base_url))
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(mapping::map_send_error)?;

        mapping::map_empty_response(response.status())
    }

    pub(super) async fn provisioner_status_request(
        &self,
        status_url: &str,
        bearer_token: &str,
    ) -> Result<RunpodProvisionerStatus, RunpodProviderError> {
        let response = self
            .http
            .get(status_url)
            .bearer_auth(bearer_token)
            .send()
            .await
            .map_err(|_| provisioner_unavailable())?;

        map_provisioner_http_status(response.status())?;
        let status = response
            .json::<ProvisionerStatusResponse>()
            .await
            .map_err(|_| provisioner_response_invalid())?;

        map_provisioner_status_response(status)
    }
}

pub(super) fn provisioner_status_url(pod_id: &str) -> String {
    format!("https://{pod_id}-{PROVISIONER_PORT}.proxy.runpod.net/status")
}

#[derive(Serialize)]
pub(super) struct GraphQlRequest<'a> {
    query: &'a str,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlResponse<T> {
    pub(super) data: Option<T>,
    #[serde(default)]
    pub(super) errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlError {
    pub(super) message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunpodIdentityData {
    pub(super) myself: Option<RunpodIdentity>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunpodIdentity {
    pub(super) email: String,
    #[serde(rename = "apiKeys")]
    pub(super) api_keys: Vec<RunpodApiKey>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RunpodApiKey {
    pub(super) id: Option<String>,
    #[serde(rename = "isActive")]
    pub(super) is_active: bool,
}

#[derive(Debug, Deserialize)]
struct ProvisionerStatusResponse {
    status: String,
    error: Option<ProvisionerWorkerErrorResponse>,
}

#[derive(Debug, Deserialize)]
struct ProvisionerWorkerErrorResponse {
    #[serde(alias = "code")]
    code: String,
    #[serde(alias = "message")]
    message: String,
}

fn map_provisioner_status_response(
    response: ProvisionerStatusResponse,
) -> Result<RunpodProvisionerStatus, RunpodProviderError> {
    match response.status.as_str() {
        STATUS_IDLE => Ok(RunpodProvisionerStatus::Pending),
        STATUS_RUNNING => Ok(RunpodProvisionerStatus::Running),
        STATUS_SUCCEEDED => Ok(RunpodProvisionerStatus::Succeeded),
        STATUS_FAILED => {
            let error = response.error.ok_or_else(provisioner_response_invalid)?;
            Ok(RunpodProvisionerStatus::Failed {
                message: format!("{}: {}", error.code, error.message),
            })
        }
        _ => Err(provisioner_response_invalid()),
    }
}

fn map_provisioner_http_status(status: StatusCode) -> Result<(), RunpodProviderError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(provisioner_response_invalid()),
        StatusCode::CONFLICT => Err(provisioner_failed()),
        _ => Err(provisioner_unavailable()),
    }
}

fn provisioner_unavailable() -> RunpodProviderError {
    RunpodProviderError::ProvisionerWorkerUnavailable
}

fn provisioner_response_invalid() -> RunpodProviderError {
    RunpodProviderError::ProvisionerWorkerResponseInvalid
}

fn provisioner_failed() -> RunpodProviderError {
    RunpodProviderError::ProvisionerWorkerFailed {
        message: "provisioner worker failed".to_string(),
    }
}
