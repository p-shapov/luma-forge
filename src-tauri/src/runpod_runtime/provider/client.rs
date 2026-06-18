use std::sync::Arc;

use crate::{
    domain::{runpod::RunpodPlacementOptions, workflow_preset::ModelAsset},
    shared::AppFuture,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::Instrument;

use super::super::errors::{
    hugging_face_api_key_unavailable, runpod_api_key_unavailable, RunpodRuntimeError,
};
use crate::secrets::{ApiKeyIdentityProvider, SecretStore, SecretsService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunpodProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodNetworkVolumeParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRunpodProvisionerPodParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub requires_hugging_face_api_key: bool,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessTemplateParams {
    pub workspace_id: String,
    pub endpoint_image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessEndpointParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub network_volume_id: String,
    pub template_id: String,
}

pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>>;

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;
}

use super::{
    config::{
        NETWORK_VOLUME_MAX_SIZE_GB, PROVISIONER_PORT, RUNPOD_GRAPHQL_URL, RUNPOD_REST_BASE_URL,
    },
    mapping::{
        self, CreateNetworkVolumeRequest, CreateProvisionerPodRequest,
        CreateServerlessEndpointRequest, CreateServerlessTemplateRequest, EndpointResponse,
        GraphqlResponse, NetworkVolumeResponse, PlacementQueryData, PodResponse, TemplateResponse,
    },
};

const STATUS_IDLE: &str = "idle";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

pub struct RunpodRuntimeProvider<RS, RI, HS, HI> {
    http: reqwest::Client,
    rest_base_url: String,
    graphql_url: String,
    runpod_secrets: Arc<SecretsService<RS, RI>>,
    hugging_face_secrets: SecretsService<HS, HI>,
}

impl<RS, RI, HS, HI> RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore + 'static,
    RI: ApiKeyIdentityProvider + 'static,
    HS: SecretStore + 'static,
    HI: ApiKeyIdentityProvider + 'static,
{
    pub fn new(
        runpod_secrets: SecretsService<RS, RI>,
        hugging_face_secrets: SecretsService<HS, HI>,
    ) -> Self {
        let http = reqwest::Client::new();
        let runpod_secrets = Arc::new(runpod_secrets);
        Self {
            http: http.clone(),
            rest_base_url: RUNPOD_REST_BASE_URL.to_string(),
            graphql_url: RUNPOD_GRAPHQL_URL.to_string(),
            runpod_secrets,
            hugging_face_secrets,
        }
    }
}

impl<RS, RI, HS, HI> RunpodRuntimeClient for RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>> {
        Box::pin(
            async move {
                let mut options = self.placement_options_request().await?;
                options.max_volume_size_gb = Some(NETWORK_VOLUME_MAX_SIZE_GB);
                Ok(options)
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "placement_options"
            )),
        )
    }

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let volume_size_gb = params.size_gb;
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_network_volume",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            volume_size_gb = volume_size_gb
        );
        Box::pin(
            async move {
                let response: NetworkVolumeResponse = self
                    .post_rest(
                        "/networkvolumes",
                        &mapping::network_volume_create_body(&network_volume_request(params)),
                    )
                    .await?;

                Ok(response.id)
            }
            .instrument(span),
        )
    }

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move {
                self.delete_rest(&format!("/networkvolumes/{network_volume_id}"))
                    .await
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "delete_network_volume",
                network_volume_id = %network_volume_id
            )),
        )
    }

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let network_volume_id = params.network_volume_id.clone();
        let requires_hugging_face_api_key = params.requires_hugging_face_api_key;
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "start_provisioner_pod",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            network_volume_id = %network_volume_id,
            requires_hugging_face_api_key = requires_hugging_face_api_key
        );
        Box::pin(
            async move {
                let bearer_token = self.workspace_bearer_token(&params.workspace_id).await?;
                let hugging_face_api_key = self.hugging_face_api_key(&params).await?;
                let request = provisioner_pod_request(params, bearer_token, hugging_face_api_key);
                let body = mapping::provisioner_pod_create_body(&request)
                    .map_err(RunpodRuntimeError::from)?;
                let pod: PodResponse = self.post_rest("/pods", &body).await?;

                Ok(pod.id)
            }
            .instrument(span),
        )
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move {
                self.delete_rest(&format!("/pods/{provisioner_pod_id}"))
                    .await
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "terminate_provisioner_pod",
                provisioner_pod_id = %provisioner_pod_id
            )),
        )
    }

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>> {
        Box::pin(
            async move {
                let bearer_token = self.workspace_bearer_token(workspace_id).await?;

                self.provisioner_status_request(
                    &provisioner_status_url(provisioner_pod_id),
                    &bearer_token,
                )
                .await
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "get_provisioner_status",
                workspace_id = %workspace_id,
                provisioner_pod_id = %provisioner_pod_id
            )),
        )
    }

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_serverless_template",
            workspace_id = %workspace_id
        );
        Box::pin(
            async move {
                let response: TemplateResponse = self
                    .post_rest(
                        "/templates",
                        &mapping::endpoint_template_create_body(&serverless_template_request(
                            params,
                        )),
                    )
                    .await?;

                Ok(response.id)
            }
            .instrument(span),
        )
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let gpu_type_id = params.gpu_type_id.clone();
        let network_volume_id = params.network_volume_id.clone();
        let template_id = params.template_id.clone();
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_serverless_endpoint",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            gpu_type_id = %gpu_type_id,
            network_volume_id = %network_volume_id,
            template_id = %template_id
        );
        Box::pin(
            async move {
                let response: EndpointResponse = self
                    .post_rest(
                        "/endpoints",
                        &mapping::endpoint_create_body(&serverless_endpoint_request(params)),
                    )
                    .await?;

                Ok(response.id)
            }
            .instrument(span),
        )
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.delete_rest(&format!("/endpoints/{endpoint_id}")).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "delete_serverless_endpoint",
                    endpoint_id = %endpoint_id
                ),
            ),
        )
    }

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.delete_rest(&format!("/templates/{template_id}")).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "delete_template",
                    template_id = %template_id
                ),
            ),
        )
    }
}

impl<RS, RI, HS, HI> RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    async fn workspace_bearer_token(
        &self,
        workspace_id: &str,
    ) -> Result<String, RunpodRuntimeError> {
        self.runpod_secrets
            .hmac_sha256_hex(workspace_id)
            .await
            .map_err(runpod_api_key_unavailable)
    }

    async fn hugging_face_api_key(
        &self,
        params: &StartRunpodProvisionerPodParams,
    ) -> Result<Option<String>, RunpodRuntimeError> {
        if !params.requires_hugging_face_api_key {
            return Ok(None);
        }

        self.hugging_face_secrets
            .retrieve()
            .await
            .map_err(hugging_face_api_key_unavailable)
            .map(|secret| Some(secret.expose_secret().to_string()))
    }

    async fn runpod_api_key(&self) -> Result<String, RunpodRuntimeError> {
        self.runpod_secrets
            .retrieve()
            .await
            .map_err(runpod_api_key_unavailable)
            .map(|secret| secret.expose_secret().to_string())
    }

    async fn placement_options_request(
        &self,
    ) -> Result<RunpodPlacementOptions, RunpodRuntimeError> {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .post(&self.graphql_url)
            .bearer_auth(&api_key)
            .json(&mapping::placement_graphql_request())
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodRuntimeError::from)?;

        let response: GraphqlResponse<PlacementQueryData> = mapping::parse_json_response(response)
            .await
            .map_err(RunpodRuntimeError::from)?;

        mapping::map_placement_response(response).map_err(RunpodRuntimeError::from)
    }

    async fn post_rest<B, T>(&self, path: &str, body: &B) -> Result<T, RunpodRuntimeError>
    where
        B: Serialize + ?Sized,
        T: for<'de> Deserialize<'de>,
    {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .post(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .json(body)
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodRuntimeError::from)?;

        mapping::parse_json_response(response)
            .await
            .map_err(RunpodRuntimeError::from)
    }

    async fn delete_rest(&self, path: &str) -> Result<(), RunpodRuntimeError> {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .delete(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodRuntimeError::from)?;

        mapping::map_empty_response(response.status()).map_err(RunpodRuntimeError::from)
    }

    async fn provisioner_status_request(
        &self,
        status_url: &str,
        bearer_token: &str,
    ) -> Result<RunpodProvisionerStatus, RunpodRuntimeError> {
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

fn network_volume_request(params: CreateRunpodNetworkVolumeParams) -> CreateNetworkVolumeRequest {
    CreateNetworkVolumeRequest {
        datacenter_id: params.data_center_id,
        name: mapping::network_volume_name(&params.workspace_id),
        size_gb: params.size_gb,
    }
}

fn provisioner_pod_request(
    params: StartRunpodProvisionerPodParams,
    bearer_token: String,
    hugging_face_api_key: Option<String>,
) -> CreateProvisionerPodRequest {
    CreateProvisionerPodRequest {
        datacenter_id: params.data_center_id,
        name: mapping::provisioner_pod_name(&params.workspace_id),
        image_ref: params.provisioner_image_ref,
        network_volume_id: params.network_volume_id,
        bearer_token,
        required_model_assets: params.required_model_assets,
        hugging_face_api_key,
    }
}

fn serverless_template_request(
    params: CreateRunpodServerlessTemplateParams,
) -> CreateServerlessTemplateRequest {
    CreateServerlessTemplateRequest {
        name: mapping::endpoint_template_name(&params.workspace_id),
        image_ref: params.endpoint_image_ref,
    }
}

fn serverless_endpoint_request(
    params: CreateRunpodServerlessEndpointParams,
) -> CreateServerlessEndpointRequest {
    CreateServerlessEndpointRequest {
        datacenter_id: params.data_center_id,
        gpu_id: params.gpu_type_id,
        name: mapping::endpoint_name(&params.workspace_id),
        template_id: params.template_id,
        network_volume_id: params.network_volume_id,
    }
}

fn provisioner_status_url(pod_id: &str) -> String {
    format!("https://{pod_id}-{PROVISIONER_PORT}.proxy.runpod.net/status")
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
) -> Result<RunpodProvisionerStatus, RunpodRuntimeError> {
    match response.status.as_str() {
        STATUS_IDLE => Ok(RunpodProvisionerStatus::Pending),
        STATUS_RUNNING => Ok(RunpodProvisionerStatus::Running),
        STATUS_SUCCEEDED => Ok(RunpodProvisionerStatus::Succeeded),
        STATUS_FAILED => {
            let error = response.error.ok_or_else(provisioner_response_invalid)?;
            Err(RunpodRuntimeError::ProvisionerWorkerFailed {
                message: format!("{}: {}", error.code, error.message),
            })
        }
        _ => Err(provisioner_response_invalid()),
    }
}

fn map_provisioner_http_status(status: StatusCode) -> Result<(), RunpodRuntimeError> {
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::UNAUTHORIZED => Err(provisioner_response_invalid()),
        StatusCode::CONFLICT => Err(provisioner_failed()),
        _ => Err(provisioner_unavailable()),
    }
}

fn provisioner_unavailable() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerUnavailable {
        message: "provisioner worker is unavailable".to_string(),
    }
}

fn provisioner_response_invalid() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerResponseInvalid {
        message: "provisioner worker response is invalid".to_string(),
    }
}

fn provisioner_failed() -> RunpodRuntimeError {
    RunpodRuntimeError::ProvisionerWorkerFailed {
        message: "provisioner worker failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_provisioner_status_response_maps_lifecycle_statuses() {
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "idle".to_string(),
                error: None,
            }),
            Ok(RunpodProvisionerStatus::Pending)
        );
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "running".to_string(),
                error: None,
            }),
            Ok(RunpodProvisionerStatus::Running)
        );
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "succeeded".to_string(),
                error: None,
            }),
            Ok(RunpodProvisionerStatus::Succeeded)
        );
    }

    #[test]
    fn map_provisioner_status_response_maps_worker_failure_details() {
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: Some(ProvisionerWorkerErrorResponse {
                    code: "asset_download_failed".to_string(),
                    message: "download failed".to_string(),
                }),
            }),
            Err(RunpodRuntimeError::ProvisionerWorkerFailed {
                message: "asset_download_failed: download failed".to_string(),
            })
        );
    }

    #[test]
    fn map_provisioner_status_response_rejects_malformed_responses() {
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "failed".to_string(),
                error: None,
            }),
            Err(provisioner_response_invalid())
        );
        assert_eq!(
            map_provisioner_status_response(ProvisionerStatusResponse {
                status: "other".to_string(),
                error: None,
            }),
            Err(provisioner_response_invalid())
        );
    }

    #[test]
    fn map_provisioner_http_status_maps_worker_errors() {
        assert_eq!(map_provisioner_http_status(StatusCode::OK), Ok(()));
        assert_eq!(
            map_provisioner_http_status(StatusCode::UNAUTHORIZED),
            Err(provisioner_response_invalid())
        );
        assert_eq!(
            map_provisioner_http_status(StatusCode::CONFLICT),
            Err(provisioner_failed())
        );
        assert_eq!(
            map_provisioner_http_status(StatusCode::INTERNAL_SERVER_ERROR),
            Err(provisioner_unavailable())
        );
    }
}
