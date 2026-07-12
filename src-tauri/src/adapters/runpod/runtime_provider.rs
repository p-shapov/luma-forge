use std::{collections::HashMap, time::Duration};

use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha256;
use tokio::time::Instant;

use crate::{
    application::runtimes::runpod::{
        CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodRuntimeProvider,
        RunpodRuntimeProviderError, StartProvisionerPod,
    },
    infra::clients::{
        runpod::{
            generated::{
                EndpointCreateInputComputeType, EndpointCreateInputDataCenterIdsItem,
                EndpointCreateInputGpuTypeIdsItem, PodCreateInputComputeType,
                PodCreateInputDataCenterIdsItem,
            },
            EndpointCreateInput, NetworkVolumeCreateInput, PodCreateInput, RunpodClient,
            TemplateCreateInput,
        },
        NetworkError,
    },
};

const RESOURCE_PREFIX: &str = "luma-forge";
const PROVISIONER_PORT: &str = "8000/http";
const ENDPOINT_WORKERS_MIN: i64 = 0;
const ENDPOINT_WORKERS_MAX: i64 = 1;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const POLL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct RunpodRuntimeProviderAdapter {
    client: RunpodClient,
}

impl RunpodRuntimeProviderAdapter {
    pub fn new(client: RunpodClient) -> Self {
        Self { client }
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
        self.client
            .create_network_volume(
                api_key,
                NetworkVolumeCreateInput {
                    data_center_id: command.datacenter_id,
                    name: resource_name(&command.workspace_id, "volume"),
                    size: command
                        .size_gb
                        .try_into()
                        .map_err(|_| RunpodRuntimeProviderError::Unavailable)?,
                },
            )
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
        let bearer_token = derive_bearer_token(api_key, &command.workspace_id)?;
        let mut env = HashMap::from([
            (
                "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_owned(),
                bearer_token,
            ),
            (
                "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS".to_owned(),
                command.required_model_assets.to_string(),
            ),
        ]);
        if let Some(api_key) = command.hugging_face_api_key {
            env.insert(
                "LUMA_FORGE_HUGGING_FACE_API_KEY".to_owned(),
                api_key.expose_secret().to_owned(),
            );
        }

        self.client
            .create_pod(
                api_key,
                PodCreateInput {
                    compute_type: Some(PodCreateInputComputeType::Cpu),
                    data_center_ids: vec![command
                        .datacenter_id
                        .parse::<PodCreateInputDataCenterIdsItem>()
                        .map_err(|_| RunpodRuntimeProviderError::Unavailable)?],
                    env,
                    image_name: Some(command.provisioner_image_ref),
                    name: Some(resource_name(&command.workspace_id, "provisioner")),
                    network_volume_id: Some(command.network_volume_id),
                    ports: vec![PROVISIONER_PORT.to_owned()],
                    ..Default::default()
                },
            )
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
        let bearer_token = SecretString::from(derive_bearer_token(api_key, workspace_id)?);
        let deadline = Instant::now() + POLL_TIMEOUT;

        loop {
            if Instant::now() >= deadline {
                return Err(RunpodRuntimeProviderError::Unavailable);
            }

            let response = self
                .client
                .provisioner_status(&bearer_token, pod_id)
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
        cleanup(self.client.delete_pod(api_key, pod_id).await)
    }

    #[diagnostic(show_output, show_error)]
    async fn create_template(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] command: CreateTemplate,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.client
            .create_template(
                api_key,
                TemplateCreateInput {
                    category: None,
                    container_disk_in_gb: None,
                    container_registry_auth_id: None,
                    docker_entrypoint: Vec::new(),
                    docker_start_cmd: Vec::new(),
                    env: HashMap::new(),
                    image_name: command.image_ref,
                    is_public: Some(false),
                    is_serverless: Some(true),
                    name: resource_name(&command.workspace_id, "template"),
                    ports: Vec::new(),
                    readme: None,
                    volume_in_gb: None,
                    volume_mount_path: None,
                },
            )
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
        self.client
            .create_endpoint(
                api_key,
                EndpointCreateInput {
                    allowed_cuda_versions: Vec::new(),
                    compute_type: Some(EndpointCreateInputComputeType::Gpu),
                    cpu_flavor_ids: Vec::new(),
                    data_center_ids: vec![command
                        .datacenter_id
                        .parse::<EndpointCreateInputDataCenterIdsItem>()
                        .map_err(|_| RunpodRuntimeProviderError::Unavailable)?],
                    execution_timeout_ms: None,
                    flashboot: None,
                    gpu_count: None,
                    gpu_type_ids: vec![command
                        .gpu_id
                        .parse::<EndpointCreateInputGpuTypeIdsItem>()
                        .map_err(|_| RunpodRuntimeProviderError::Unavailable)?],
                    idle_timeout: None,
                    min_cuda_version: None,
                    name: Some(resource_name(&command.workspace_id, "endpoint")),
                    network_volume_id: Some(command.network_volume_id),
                    network_volume_ids: Vec::new(),
                    scaler_type: None,
                    scaler_value: None,
                    template_id: command.template_id,
                    vcpu_count: None,
                    workers_max: Some(ENDPOINT_WORKERS_MAX),
                    workers_min: Some(ENDPOINT_WORKERS_MIN),
                },
            )
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
        cleanup(self.client.delete_endpoint(api_key, id).await)
    }

    #[diagnostic(show_error)]
    async fn delete_template(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(self.client.delete_template(api_key, id).await)
    }

    #[diagnostic(show_error)]
    async fn delete_network_volume(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
        #[diagnostic(show)] id: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        cleanup(self.client.delete_network_volume(api_key, id).await)
    }
}

fn resource_name(workspace_id: &str, resource: &str) -> String {
    format!("{RESOURCE_PREFIX}-{workspace_id}-{resource}")
}

fn derive_bearer_token(
    api_key: &SecretString,
    workspace_id: &str,
) -> Result<String, RunpodRuntimeProviderError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(api_key.expose_secret().as_bytes())
        .map_err(|_| RunpodRuntimeProviderError::Unavailable)?;
    mac.update(workspace_id.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
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
