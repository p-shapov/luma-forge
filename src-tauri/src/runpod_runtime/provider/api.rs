use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    domain::{runpod::RunpodPlacementOptions, workflow_preset::ModelAsset},
    runpod_runtime::errors::{runpod_api_key_unavailable, RunpodRuntimeError},
    secrets_storage::{ApiKeyIdentityProvider, SecretStore, SecretsStorageService},
    shared::AppFuture,
};

use super::{
    mapping::{
        endpoint_create_body, endpoint_template_create_body, map_empty_response,
        map_placement_response, map_send_error, network_volume_create_body, parse_json_response,
        placement_graphql_request, provisioner_pod_create_body, EndpointResponse, GraphqlResponse,
        NetworkVolumeResponse, PlacementQueryData, PodResponse, RunpodOperation, TemplateResponse,
    },
    RunpodEndpointKeepAliveLimits,
};

pub trait RunpodApi: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>>;

    fn create_network_volume<'a>(
        &'a self,
        request: CreateNetworkVolumeRequest,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_network_volume<'a>(
        &'a self,
        volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn create_provisioner_pod<'a>(
        &'a self,
        request: CreateProvisionerPodRequest,
    ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>>;

    fn delete_provisioner_pod<'a>(
        &'a self,
        pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn create_serverless_template<'a>(
        &'a self,
        request: CreateServerlessTemplateRequest,
    ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        request: CreateServerlessEndpointRequest,
    ) -> AppFuture<'a, Result<RunpodEndpoint, RunpodRuntimeError>>;

    fn delete_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateNetworkVolumeRequest {
    pub datacenter_id: String,
    pub name: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProvisionerPodRequest {
    pub datacenter_id: String,
    pub name: String,
    pub image_ref: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub bearer_token: String,
    pub job_id: String,
    pub required_model_assets: Vec<ModelAsset>,
    pub hugging_face_api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateServerlessTemplateRequest {
    pub name: String,
    pub image_ref: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateServerlessEndpointRequest {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub name: String,
    pub template_id: String,
    pub network_volume_id: String,
    pub keep_alive_limits: RunpodEndpointKeepAliveLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodId {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodEndpoint {
    pub id: String,
    pub template_id: String,
    pub url: String,
}

pub struct HttpRunpodApi<S, I> {
    http: reqwest::Client,
    rest_base_url: String,
    graphql_url: String,
    secrets: Arc<SecretsStorageService<S, I>>,
}

impl<S, I> HttpRunpodApi<S, I> {
    pub fn new(
        http: reqwest::Client,
        rest_base_url: String,
        graphql_url: String,
        secrets: Arc<SecretsStorageService<S, I>>,
    ) -> Self {
        Self {
            http,
            rest_base_url,
            graphql_url,
            secrets,
        }
    }
}

impl<S, I> HttpRunpodApi<S, I>
where
    S: SecretStore,
    I: ApiKeyIdentityProvider,
{
    async fn post_rest<B, T>(
        &self,
        path: &str,
        body: &B,
        operation: RunpodOperation,
    ) -> Result<T, RunpodRuntimeError>
    where
        B: Serialize + ?Sized,
        T: for<'de> Deserialize<'de>,
    {
        let api_key = self.api_key().await?;
        let response = self
            .http
            .post(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .json(body)
            .send()
            .await
            .map_err(map_send_error)
            .map_err(RunpodRuntimeError::from)?;

        parse_json_response(response, operation)
            .await
            .map_err(RunpodRuntimeError::from)
    }

    async fn delete_rest(
        &self,
        path: &str,
        operation: RunpodOperation,
    ) -> Result<(), RunpodRuntimeError> {
        let api_key = self.api_key().await?;
        let response = self
            .http
            .delete(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(map_send_error)
            .map_err(RunpodRuntimeError::from)?;

        map_empty_response(response.status(), operation).map_err(RunpodRuntimeError::from)
    }

    async fn api_key(&self) -> Result<String, RunpodRuntimeError> {
        self.secrets
            .retrieve()
            .await
            .map_err(runpod_api_key_unavailable)
            .map(|secret| secret.expose_secret().to_string())
    }
}

impl<S, I> RunpodApi for HttpRunpodApi<S, I>
where
    S: SecretStore,
    I: ApiKeyIdentityProvider,
{
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>> {
        Box::pin(async move {
            let api_key = self.api_key().await?;
            let response = self
                .http
                .post(&self.graphql_url)
                .bearer_auth(&api_key)
                .json(&placement_graphql_request())
                .send()
                .await
                .map_err(map_send_error)
                .map_err(RunpodRuntimeError::from)?;

            let response: GraphqlResponse<PlacementQueryData> =
                parse_json_response(response, RunpodOperation::PlacementOptions)
                    .await
                    .map_err(RunpodRuntimeError::from)?;

            map_placement_response(response).map_err(RunpodRuntimeError::from)
        })
    }

    fn create_network_volume<'a>(
        &'a self,
        request: CreateNetworkVolumeRequest,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        Box::pin(async move {
            let response: NetworkVolumeResponse = self
                .post_rest(
                    "/networkvolumes",
                    &network_volume_create_body(&request),
                    RunpodOperation::CreateNetworkVolume,
                )
                .await?;

            Ok(response.id)
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            self.delete_rest(
                &format!("/networkvolumes/{volume_id}"),
                RunpodOperation::DeleteNetworkVolume,
            )
            .await
        })
    }

    fn create_provisioner_pod<'a>(
        &'a self,
        request: CreateProvisionerPodRequest,
    ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>> {
        Box::pin(async move {
            let body = provisioner_pod_create_body(&request).map_err(RunpodRuntimeError::from)?;
            let response: PodResponse = self
                .post_rest("/pods", &body, RunpodOperation::CreateProvisionerPod)
                .await?;

            Ok(RunpodId { id: response.id })
        })
    }

    fn delete_provisioner_pod<'a>(
        &'a self,
        pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            self.delete_rest(
                &format!("/pods/{pod_id}"),
                RunpodOperation::DeleteProvisionerPod,
            )
            .await
        })
    }

    fn create_serverless_template<'a>(
        &'a self,
        request: CreateServerlessTemplateRequest,
    ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>> {
        Box::pin(async move {
            let response: TemplateResponse = self
                .post_rest(
                    "/templates",
                    &endpoint_template_create_body(&request),
                    RunpodOperation::CreateTemplate,
                )
                .await?;

            Ok(RunpodId { id: response.id })
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        request: CreateServerlessEndpointRequest,
    ) -> AppFuture<'a, Result<RunpodEndpoint, RunpodRuntimeError>> {
        Box::pin(async move {
            let endpoint_response: EndpointResponse = self
                .post_rest(
                    "/endpoints",
                    &endpoint_create_body(&request),
                    RunpodOperation::CreateEndpoint,
                )
                .await?;

            Ok(RunpodEndpoint {
                id: endpoint_response.id,
                template_id: request.template_id,
                url: endpoint_response.url.unwrap_or_default(),
            })
        })
    }

    fn delete_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            self.delete_rest(
                &format!("/endpoints/{endpoint_id}"),
                RunpodOperation::DeleteEndpoint,
            )
            .await
        })
    }

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            self.delete_rest(
                &format!("/templates/{template_id}"),
                RunpodOperation::DeleteTemplate,
            )
            .await
        })
    }
}
