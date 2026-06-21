use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::LifecycleOperation, runpod::RunpodPlacementOptions,
        workflow_preset::ModelAsset, workspace::Workspace,
    },
    provider::errors::ProviderApiError,
    runtime_catalog::RuntimeCatalogRepository,
    secrets::SecretsStorageError,
    secrets::{ApiKeyIdentityProvider, SecretStore, SecretsService},
    workflow_catalog::WorkflowCatalogRepository,
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

#[async_trait::async_trait]
pub trait RunpodRuntimeClient: Send + Sync {
    async fn placement_options(&self) -> Result<RunpodPlacementOptions, RunpodProviderError>;

    async fn create_network_volume(
        &self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> Result<String, RunpodProviderError>;

    async fn delete_network_volume(
        &self,
        network_volume_id: &str,
    ) -> Result<(), RunpodProviderError>;

    async fn start_provisioner_pod(
        &self,
        params: StartRunpodProvisionerPodParams,
    ) -> Result<String, RunpodProviderError>;

    async fn terminate_provisioner_pod(
        &self,
        provisioner_pod_id: &str,
    ) -> Result<(), RunpodProviderError>;

    async fn get_provisioner_status(
        &self,
        workspace_id: &str,
        provisioner_pod_id: &str,
    ) -> Result<RunpodProvisionerStatus, RunpodProviderError>;

    async fn create_serverless_template(
        &self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> Result<String, RunpodProviderError>;

    async fn create_serverless_endpoint(
        &self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> Result<String, RunpodProviderError>;

    async fn delete_serverless_endpoint(
        &self,
        endpoint_id: &str,
    ) -> Result<(), RunpodProviderError>;

    async fn delete_template(&self, template_id: &str) -> Result<(), RunpodProviderError>;
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

#[async_trait::async_trait]
impl<RS, RI, HS, HI> RunpodRuntimeClient for RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    async fn placement_options(&self) -> Result<RunpodPlacementOptions, RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        let mut options = self
            .client
            .placement_options_request(&api_key)
            .await
            .map_err(RunpodProviderError::from)?;
        options.max_volume_size_gb = Some(NETWORK_VOLUME_MAX_SIZE_GB);
        Ok(options)
    }

    async fn create_network_volume(
        &self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> Result<String, RunpodProviderError> {
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

    async fn delete_network_volume(
        &self,
        network_volume_id: &str,
    ) -> Result<(), RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        self.client
            .delete_network_volume(&api_key, network_volume_id)
            .await
            .map_err(RunpodProviderError::from)
    }

    async fn start_provisioner_pod(
        &self,
        params: StartRunpodProvisionerPodParams,
    ) -> Result<String, RunpodProviderError> {
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

    async fn terminate_provisioner_pod(
        &self,
        provisioner_pod_id: &str,
    ) -> Result<(), RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        self.client
            .delete_pod(&api_key, provisioner_pod_id)
            .await
            .map_err(RunpodProviderError::from)
    }

    async fn get_provisioner_status(
        &self,
        workspace_id: &str,
        provisioner_pod_id: &str,
    ) -> Result<RunpodProvisionerStatus, RunpodProviderError> {
        let bearer_token = self.workspace_bearer_token(workspace_id).await?;
        self.client
            .provisioner_status_request(&provisioner_status_url(provisioner_pod_id), &bearer_token)
            .await
    }

    async fn create_serverless_template(
        &self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> Result<String, RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        let body =
            mapping::endpoint_template_create_body(&params.workspace_id, params.endpoint_image_ref);
        let response: TemplateResponse = self
            .client
            .create_template(&api_key, &body)
            .await
            .map_err(RunpodProviderError::from)?;

        Ok(response.id)
    }

    async fn create_serverless_endpoint(
        &self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> Result<String, RunpodProviderError> {
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

    async fn delete_serverless_endpoint(
        &self,
        endpoint_id: &str,
    ) -> Result<(), RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        self.client
            .delete_endpoint(&api_key, endpoint_id)
            .await
            .map_err(RunpodProviderError::from)
    }

    async fn delete_template(&self, template_id: &str) -> Result<(), RunpodProviderError> {
        let api_key = self.runpod_api_key().await?;
        self.client
            .delete_template(&api_key, template_id)
            .await
            .map_err(RunpodProviderError::from)
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
    workflow_catalog: Arc<dyn WorkflowCatalogRepository>,
    runtime_catalog: Arc<dyn RuntimeCatalogRepository>,
}

impl RunpodWorkspaceRuntime {
    pub fn new(
        runpod_client: Arc<dyn RunpodRuntimeClient>,
        workflow_catalog: Arc<dyn WorkflowCatalogRepository>,
        runtime_catalog: Arc<dyn RuntimeCatalogRepository>,
    ) -> Self {
        Self {
            runpod_client,
            workflow_catalog,
            runtime_catalog,
        }
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
            WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed { message })
        }
    }
}

#[async_trait::async_trait]
impl WorkspaceRuntime for RunpodWorkspaceRuntime {
    async fn provision<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, WorkspaceError> {
        super::provision::provision_workspace(
            context,
            self.runpod_client.as_ref(),
            self.workflow_catalog.as_ref(),
            self.runtime_catalog.as_ref(),
            operation,
            workspace,
        )
        .await
    }

    async fn cleanup<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, WorkspaceError> {
        super::cleanup::cleanup_workspace(
            context,
            self.runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
    }

    async fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, WorkspaceError> {
        super::delete::delete_workspace(context, self.runpod_client.as_ref(), operation, workspace)
            .await
    }
}
