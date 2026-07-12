use std::time::Duration;

use secrecy::SecretString;
use tokio::time::Instant;

use crate::{
    application::runtimes::runpod::{
        CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodRuntimeProvider,
        RunpodRuntimeProviderError, StartProvisionerPod,
    },
    providers::{
        runpod::{
            CreateEndpointRequest, CreateNetworkVolumeRequest, CreatePodRequest,
            CreateTemplateRequest, DeleteEndpointRequest, DeleteNetworkVolumeRequest,
            DeletePodRequest, DeleteTemplateRequest, ProvisionerStatusRequest, RunpodProvider,
        },
        NetworkError,
    },
};

const ENDPOINT_WORKERS_MIN: i64 = 0;
const ENDPOINT_WORKERS_MAX: i64 = 1;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const POLL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct RunpodRuntimeProviderAdapter {
    provider: RunpodProvider,
}

impl RunpodRuntimeProviderAdapter {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            provider: RunpodProvider::new()?,
        })
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl RunpodRuntimeProvider for RunpodRuntimeProviderAdapter {
    #[diagnostic(show_output, show_error)]
    async fn create_network_volume(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] command: CreateNetworkVolume,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.provider
            .create_network_volume(CreateNetworkVolumeRequest {
                credential: api_key.clone(),
                workspace_id: command.workspace_id,
                datacenter_id: command.datacenter_id,
                size_gb: command
                    .size_gb
                    .try_into()
                    .map_err(|_| RunpodRuntimeProviderError::Unavailable)?,
            })
            .await
            .map_err(map_error)?
            .id
            .ok_or(RunpodRuntimeProviderError::Unavailable)
    }

    #[diagnostic(show_output, show_error)]
    async fn start_provisioner_pod(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] command: StartProvisionerPod,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.provider
            .create_pod(CreatePodRequest {
                credential: api_key.clone(),
                hugging_face_credential: command.hugging_face_api_key,
                workspace_id: command.workspace_id,
                datacenter_id: command.datacenter_id,
                provisioner_image_ref: command.provisioner_image_ref,
                network_volume_id: command.network_volume_id,
                required_model_assets: command.required_model_assets,
            })
            .await
            .map_err(map_error)?
            .id
            .ok_or(RunpodRuntimeProviderError::Unavailable)
    }

    #[diagnostic(show_error)]
    async fn wait_for_provisioner(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] workspace_id: &str,
        #[diagnostic(show)] pod_id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        let deadline = Instant::now() + POLL_TIMEOUT;

        loop {
            if Instant::now() >= deadline {
                return Err(RunpodRuntimeProviderError::Unavailable);
            }

            let response = self
                .provider
                .provisioner_status(ProvisionerStatusRequest {
                    credential: api_key.clone(),
                    workspace_id: workspace_id.to_owned(),
                    pod_id: pod_id.to_owned(),
                })
                .await
                .map_err(map_error)?;
            match response.status.as_str() {
                "succeeded" => return Ok(()),
                "failed" => return Err(RunpodRuntimeProviderError::ProvisionerFailed),
                "idle" | "running" => tokio::time::sleep(POLL_INTERVAL).await,
                _ => return Err(RunpodRuntimeProviderError::Unavailable),
            }
        }
    }

    #[diagnostic(show_error)]
    async fn terminate_provisioner_pod(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] pod_id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(
            self.provider
                .delete_pod(DeletePodRequest {
                    credential: api_key.clone(),
                    id: pod_id.to_owned(),
                })
                .await,
        )
    }

    #[diagnostic(show_output, show_error)]
    async fn create_template(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] command: CreateTemplate,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.provider
            .create_template(CreateTemplateRequest {
                credential: api_key.clone(),
                workspace_id: command.workspace_id,
                image_ref: command.image_ref,
            })
            .await
            .map_err(map_error)?
            .id
            .ok_or(RunpodRuntimeProviderError::Unavailable)
    }

    #[diagnostic(show_output, show_error)]
    async fn create_endpoint(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] command: CreateEndpoint,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.provider
            .create_endpoint(CreateEndpointRequest {
                credential: api_key.clone(),
                workspace_id: command.workspace_id,
                datacenter_id: command.datacenter_id,
                gpu_id: command.gpu_id,
                network_volume_id: command.network_volume_id,
                template_id: command.template_id,
                workers_min: ENDPOINT_WORKERS_MIN,
                workers_max: ENDPOINT_WORKERS_MAX,
            })
            .await
            .map_err(map_error)?
            .id
            .ok_or(RunpodRuntimeProviderError::Unavailable)
    }

    #[diagnostic(show_error)]
    async fn delete_endpoint(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(
            self.provider
                .delete_endpoint(DeleteEndpointRequest {
                    credential: api_key.clone(),
                    id: id.to_owned(),
                })
                .await,
        )
    }

    #[diagnostic(show_error)]
    async fn delete_template(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(
            self.provider
                .delete_template(DeleteTemplateRequest {
                    credential: api_key.clone(),
                    id: id.to_owned(),
                })
                .await,
        )
    }

    #[diagnostic(show_error)]
    async fn delete_network_volume(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(
            self.provider
                .delete_network_volume(DeleteNetworkVolumeRequest {
                    credential: api_key.clone(),
                    id: id.to_owned(),
                })
                .await,
        )
    }
}

fn cleanup(result: Result<(), NetworkError>) -> Result<(), RunpodRuntimeProviderError> {
    match result {
        Ok(()) | Err(NetworkError::NotFound) => Ok(()),
        Err(error) => Err(map_error(error)),
    }
}

fn map_error(error: NetworkError) -> RunpodRuntimeProviderError {
    match error {
        NetworkError::Unauthorized => RunpodRuntimeProviderError::Unauthorized,
        _ => RunpodRuntimeProviderError::Unavailable,
    }
}
