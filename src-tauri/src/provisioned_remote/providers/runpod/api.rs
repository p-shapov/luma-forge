use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    domain::{
        placement::{RemoteEndpointKeepAliveLimits, RemotePlacementOptions},
        provisioned_remote::ProvisionedRemoteVolumeSnapshot,
    },
    provisioned_remote::errors::ProvisionedRemoteError,
    secrets_storage::{ApiKeyIdentityProvider, SecretStore, SecretsStorageService},
    shared::AppFuture,
};

use super::mapping::{
    endpoint_create_body, endpoint_template_create_body, map_empty_response,
    map_placement_response, map_secret_error, map_send_error, network_volume_create_body,
    parse_json_response, placement_graphql_request, provisioner_pod_create_body,
    EndpointDetailsResponse, EndpointResponse, GraphqlResponse, NetworkVolumeResponse,
    PlacementQueryData, PodResponse, RunpodOperation, TemplateResponse,
};

pub trait RunpodApi: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, ProvisionedRemoteError>>;

    fn create_network_volume<'a>(
        &'a self,
        request: CreateNetworkVolumeRequest,
    ) -> AppFuture<'a, Result<ProvisionedRemoteVolumeSnapshot, ProvisionedRemoteError>>;

    fn delete_network_volume<'a>(
        &'a self,
        volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn create_provisioner_pod<'a>(
        &'a self,
        request: CreateProvisionerPodRequest,
    ) -> AppFuture<'a, Result<RunpodId, ProvisionedRemoteError>>;

    fn delete_provisioner_pod<'a>(
        &'a self,
        pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn create_endpoint<'a>(
        &'a self,
        request: CreateEndpointRequest,
    ) -> AppFuture<'a, Result<RunpodEndpoint, ProvisionedRemoteError>>;

    fn delete_endpoint_and_template<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
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
    pub requires_hugging_face_api_key: String,
    pub required_model_assets: String,
    pub hugging_face_api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEndpointRequest {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub endpoint_name: String,
    pub template_name: String,
    pub image_ref: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub keep_alive_limits: RemoteEndpointKeepAliveLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodId {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodEndpoint {
    pub id: String,
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
    ) -> Result<T, ProvisionedRemoteError>
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
            .map_err(map_send_error)?;

        parse_json_response(response, operation).await
    }

    async fn delete_rest(
        &self,
        path: &str,
        operation: RunpodOperation,
    ) -> Result<(), ProvisionedRemoteError> {
        let api_key = self.api_key().await?;
        let response = self
            .http
            .delete(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(map_send_error)?;

        map_empty_response(response.status(), operation)
    }

    async fn get_rest<T>(
        &self,
        path: &str,
        operation: RunpodOperation,
    ) -> Result<T, ProvisionedRemoteError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let api_key = self.api_key().await?;
        let response = self
            .http
            .get(format!("{}{}", self.rest_base_url, path))
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(map_send_error)?;

        parse_json_response(response, operation).await
    }

    async fn api_key(&self) -> Result<String, ProvisionedRemoteError> {
        self.secrets
            .retrieve()
            .await
            .map_err(map_secret_error)
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
    ) -> AppFuture<'a, Result<RemotePlacementOptions, ProvisionedRemoteError>> {
        Box::pin(async move {
            let api_key = self.api_key().await?;
            let response = self
                .http
                .post(&self.graphql_url)
                .bearer_auth(&api_key)
                .json(&placement_graphql_request())
                .send()
                .await
                .map_err(map_send_error)?;

            let response: GraphqlResponse<PlacementQueryData> =
                parse_json_response(response, RunpodOperation::PlacementOptions).await?;

            map_placement_response(response)
        })
    }

    fn create_network_volume<'a>(
        &'a self,
        request: CreateNetworkVolumeRequest,
    ) -> AppFuture<'a, Result<ProvisionedRemoteVolumeSnapshot, ProvisionedRemoteError>> {
        Box::pin(async move {
            let response: NetworkVolumeResponse = self
                .post_rest(
                    "/networkvolumes",
                    &network_volume_create_body(&request),
                    RunpodOperation::CreateNetworkVolume,
                )
                .await?;

            Ok(ProvisionedRemoteVolumeSnapshot { id: response.id })
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
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
    ) -> AppFuture<'a, Result<RunpodId, ProvisionedRemoteError>> {
        Box::pin(async move {
            let response: PodResponse = self
                .post_rest(
                    "/pods",
                    &provisioner_pod_create_body(&request),
                    RunpodOperation::CreateProvisionerPod,
                )
                .await?;

            Ok(RunpodId { id: response.id })
        })
    }

    fn delete_provisioner_pod<'a>(
        &'a self,
        pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            self.delete_rest(
                &format!("/pods/{pod_id}"),
                RunpodOperation::DeleteProvisionerPod,
            )
            .await
        })
    }

    fn create_endpoint<'a>(
        &'a self,
        request: CreateEndpointRequest,
    ) -> AppFuture<'a, Result<RunpodEndpoint, ProvisionedRemoteError>> {
        Box::pin(async move {
            let template_response: TemplateResponse = self
                .post_rest(
                    "/templates",
                    &endpoint_template_create_body(&request),
                    RunpodOperation::CreateTemplate,
                )
                .await?;

            let endpoint_response: EndpointResponse = match self
                .post_rest(
                    "/endpoints",
                    &endpoint_create_body(&request, &template_response.id),
                    RunpodOperation::CreateEndpoint,
                )
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let _ = self
                        .delete_rest(
                            &format!("/templates/{}", template_response.id),
                            RunpodOperation::DeleteTemplate,
                        )
                        .await;
                    return Err(error);
                }
            };

            Ok(RunpodEndpoint {
                id: endpoint_response.id,
                url: endpoint_response.url,
            })
        })
    }

    fn delete_endpoint_and_template<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let endpoint: EndpointDetailsResponse = self
                .get_rest(
                    &format!("/endpoints/{endpoint_id}?includeTemplate=true"),
                    RunpodOperation::GetEndpoint,
                )
                .await?;
            let template_id = endpoint.template_id()?;

            match self
                .delete_rest(
                    &format!("/endpoints/{endpoint_id}"),
                    RunpodOperation::DeleteEndpoint,
                )
                .await
            {
                Ok(()) | Err(ProvisionedRemoteError::RemoteEndpointNotFound) => {}
                Err(error) => return Err(error),
            }

            match self
                .delete_rest(
                    &format!("/templates/{template_id}"),
                    RunpodOperation::DeleteTemplate,
                )
                .await
            {
                Ok(()) | Err(ProvisionedRemoteError::RemoteEndpointNotFound) => Ok(()),
                Err(error) => Err(error),
            }
        })
    }
}
