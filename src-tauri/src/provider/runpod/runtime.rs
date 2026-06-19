use std::sync::Arc;

use tracing::Instrument;

use crate::{
    domain::{
        lifecycle_operation::LifecycleOperation, runpod::RunpodPlacementOptions,
        workflow_preset::ModelAsset, workspace::Workspace,
    },
    secrets::SecretsStorageError,
    secrets::{ApiKeyIdentityProvider, SecretStore, SecretsService},
    shared::{ApiError, AppFuture},
    workspace::{WorkspaceError, WorkspaceRuntime, WorkspaceRuntimeContext},
};

use super::{
    client::{provisioner_status_url, RunpodApiClient},
    errors::RunpodProviderError,
    mapping::{self, EndpointResponse, NetworkVolumeResponse, PodResponse, TemplateResponse},
};

const NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;

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
    client: RunpodApiClient,
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
    ) -> Result<Self, SecretsStorageError> {
        Ok(Self {
            client: RunpodApiClient::new()?,
            runpod_secrets,
            hugging_face_secrets,
        })
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
                let api_key = self.runpod_api_key().await?;
                let mut options = self
                    .client
                    .placement_options_request(&api_key)
                    .await
                    .map_err(RunpodProviderError::from)?;
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
                let api_key = self.runpod_api_key().await?;
                let body = mapping::network_volume_create_body(
                    &params.workspace_id,
                    params.data_center_id,
                    params.size_gb,
                );
                let response: NetworkVolumeResponse = self
                    .client
                    .create_network_volume(&api_key, &body)
                    .await
                    .map_err(RunpodProviderError::from)?;

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
                let api_key = self.runpod_api_key().await?;
                self.client
                    .delete_network_volume(&api_key, network_volume_id)
                    .await
                    .map_err(RunpodProviderError::from)
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
                let api_key = self.runpod_api_key().await?;
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
                let pod: PodResponse = self
                    .client
                    .start_pod(&api_key, &body)
                    .await
                    .map_err(RunpodProviderError::from)?;

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
                let api_key = self.runpod_api_key().await?;
                self.client
                    .delete_pod(&api_key, provisioner_pod_id)
                    .await
                    .map_err(RunpodProviderError::from)
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
                self.client
                    .provisioner_status_request(
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
                let api_key = self.runpod_api_key().await?;
                let body = mapping::endpoint_template_create_body(
                    &params.workspace_id,
                    params.endpoint_image_ref,
                );
                let response: TemplateResponse = self
                    .client
                    .create_template(&api_key, &body)
                    .await
                    .map_err(RunpodProviderError::from)?;

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
                let api_key = self.runpod_api_key().await?;
                let body = mapping::endpoint_create_body(
                    &params.workspace_id,
                    params.data_center_id,
                    params.gpu_type_id,
                    params.network_volume_id,
                    params.template_id,
                );
                let response: EndpointResponse = self
                    .client
                    .create_endpoint(&api_key, &body)
                    .await
                    .map_err(RunpodProviderError::from)?;

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
            async move {
                let api_key = self.runpod_api_key().await?;
                self.client
                    .delete_endpoint(&api_key, endpoint_id)
                    .await
                    .map_err(RunpodProviderError::from)
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "delete_serverless_endpoint",
                endpoint_id = %endpoint_id
            )),
        )
    }

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
        Box::pin(
            async move {
                let api_key = self.runpod_api_key().await?;
                self.client
                    .delete_template(&api_key, template_id)
                    .await
                    .map_err(RunpodProviderError::from)
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "delete_template",
                template_id = %template_id
            )),
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
}

#[derive(Clone)]
pub struct RunpodWorkspaceRuntime {
    runpod_client: Arc<dyn RunpodRuntimeClient>,
}

impl RunpodWorkspaceRuntime {
    pub fn new(runpod_client: Arc<dyn RunpodRuntimeClient>) -> Self {
        Self { runpod_client }
    }
}

pub(super) fn map_provider_error(error: RunpodProviderError) -> WorkspaceError {
    match error {
        RunpodProviderError::ProviderApiError(error) => WorkspaceError::ProviderApiError(error),
        RunpodProviderError::RuntimeProviderApiKeyUnavailable(error) => {
            WorkspaceError::RuntimeProviderApiKeyUnavailable(error)
        }
        RunpodProviderError::WorkflowProviderApiKeyUnavailable(error) => {
            WorkspaceError::WorkflowProviderApiKeyUnavailable(error)
        }
        RunpodProviderError::ProvisionerWorkerUnavailable { message }
        | RunpodProviderError::ProvisionerWorkerResponseInvalid { message }
        | RunpodProviderError::ProvisionerWorkerFailed { message } => {
            WorkspaceError::ProviderApiError(ApiError::RequestFailed { message })
        }
    }
}

impl WorkspaceRuntime for RunpodWorkspaceRuntime {
    fn provision<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::provision::provision_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn cleanup<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::cleanup::cleanup_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::delete::delete_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }
}
