use reqwest::StatusCode;
use serde::Deserialize;
use tracing::Instrument;

use crate::{
    domain::{runpod::RunpodPlacementOptions, workflow_preset::ModelAsset},
    secrets::{ApiKeyIdentityProvider, SecretStore, SecretsService},
    shared::AppFuture,
};

use super::{
    errors::RunpodProviderError,
    mapping::{
        self, EndpointResponse, GraphqlResponse, NetworkVolumeResponse, PlacementQueryData,
        PodResponse, TemplateResponse,
    },
};

const RUNPOD_REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;
const PROVISIONER_PORT: u16 = 8000;

const STATUS_IDLE: &str = "idle";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunpodProvisionerStatus {
    Pending,
    Running,
    Succeeded,
    Failed { message: String },
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
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodProviderError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodProviderError>>;

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>>;
}

pub struct RunpodRuntimeProvider<RS, RI, HS, HI> {
    http: reqwest::Client,
    runpod_secrets: SecretsService<RS, RI>,
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
        Self {
            http: reqwest::Client::new(),
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
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodProviderError>> {
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
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
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
                        &mapping::network_volume_create_body(
                            &params.workspace_id,
                            params.data_center_id,
                            params.size_gb,
                        ),
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
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
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
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
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
                let body = mapping::provisioner_pod_create_body(
                    &params.workspace_id,
                    params.data_center_id,
                    params.provisioner_image_ref,
                    params.network_volume_id,
                    bearer_token,
                    params.required_model_assets,
                    hugging_face_api_key,
                )
                .map_err(RunpodProviderError::from)?;
                let pod: PodResponse = self.post_rest("/pods", &body).await?;

                Ok(pod.id)
            }
            .instrument(span),
        )
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
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
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodProviderError>> {
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
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
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
                        &mapping::endpoint_template_create_body(
                            &params.workspace_id,
                            params.endpoint_image_ref,
                        ),
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
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
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
                        &mapping::endpoint_create_body(
                            &params.workspace_id,
                            params.data_center_id,
                            params.gpu_type_id,
                            params.network_volume_id,
                            params.template_id,
                        ),
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
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
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
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
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
    ) -> Result<String, RunpodProviderError> {
        self.runpod_secrets
            .hmac_sha256_hex(workspace_id)
            .await
            .map_err(RunpodProviderError::RuntimeProviderApiKeyUnavailable)
    }

    async fn hugging_face_api_key(
        &self,
        params: &StartRunpodProvisionerPodParams,
    ) -> Result<Option<String>, RunpodProviderError> {
        if !params.requires_hugging_face_api_key {
            return Ok(None);
        }

        self.hugging_face_secrets
            .retrieve()
            .await
            .map_err(RunpodProviderError::WorkflowProviderApiKeyUnavailable)
            .map(|secret| Some(secret.expose_secret().to_string()))
    }

    async fn runpod_api_key(&self) -> Result<String, RunpodProviderError> {
        self.runpod_secrets
            .retrieve()
            .await
            .map_err(RunpodProviderError::RuntimeProviderApiKeyUnavailable)
            .map(|secret| secret.expose_secret().to_string())
    }

    async fn placement_options_request(
        &self,
    ) -> Result<RunpodPlacementOptions, RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .post(RUNPOD_GRAPHQL_URL)
            .bearer_auth(&api_key)
            .json(&mapping::placement_graphql_request())
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodProviderError::from)?;

        let response: GraphqlResponse<PlacementQueryData> = mapping::parse_json_response(response)
            .await
            .map_err(RunpodProviderError::from)?;

        mapping::map_placement_response(response).map_err(RunpodProviderError::from)
    }

    async fn post_rest<B, T>(&self, path: &str, body: &B) -> Result<T, RunpodProviderError>
    where
        B: serde::Serialize + ?Sized,
        T: for<'de> serde::Deserialize<'de>,
    {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .post(format!("{RUNPOD_REST_BASE_URL}{path}"))
            .bearer_auth(&api_key)
            .json(body)
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodProviderError::from)?;

        mapping::parse_json_response(response)
            .await
            .map_err(RunpodProviderError::from)
    }

    async fn delete_rest(&self, path: &str) -> Result<(), RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        let response = self
            .http
            .delete(format!("{RUNPOD_REST_BASE_URL}{path}"))
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(mapping::map_send_error)
            .map_err(RunpodProviderError::from)?;

        mapping::map_empty_response(response.status()).map_err(RunpodProviderError::from)
    }

    async fn provisioner_status_request(
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
    RunpodProviderError::ProvisionerWorkerUnavailable {
        message: "provisioner worker is unavailable".to_string(),
    }
}

fn provisioner_response_invalid() -> RunpodProviderError {
    RunpodProviderError::ProvisionerWorkerResponseInvalid {
        message: "provisioner worker response is invalid".to_string(),
    }
}

fn provisioner_failed() -> RunpodProviderError {
    RunpodProviderError::ProvisionerWorkerFailed {
        message: "provisioner worker failed".to_string(),
    }
}
