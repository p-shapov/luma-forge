use std::{collections::HashMap, future::Future, pin::Pin};

use crate::{
    domain::{
        placement::ProviderPlacementCapabilities,
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    },
    provider::{
        error::ProviderClientError,
        runpod::{
            RunPodClient, RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest,
            RunPodCreatePodRequest, RunPodCreateTemplateRequest, RunPodEndpointObservation,
            RunPodNetworkVolumeObservation, RunPodPodObservation, RunPodTemplateObservation,
        },
    },
    provider_setup::{ProviderIdentityGateway, ProviderSetupError},
    secrets::{KeyringSecretStore, SecretStore},
    workspace_provisioning::{
        CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, DiscoverProvisioningPodsInput, EndpointTemplateObservation,
        NetworkVolumeObservation, ObserveProvisioningPodInput, ProviderProvisioningGateway,
        ProvisioningPodObservation, ServerlessEndpointObservation, WorkspaceProvisioningError,
    },
    workspace_setup::{
        contracts::ProviderPlacementOptions, error::WorkspaceSetupError,
        ProviderPlacementOptionsGateway,
    },
};

const RUNPOD_VOLUME_MOUNT_PATH: &str = "/workspace";
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProviderClientRegistry<S = KeyringSecretStore> {
    secrets: S,
    runpod: RunPodClient,
}

impl<S> ProviderClientRegistry<S> {
    pub fn new(secrets: S, runpod: RunPodClient) -> Self {
        Self { secrets, runpod }
    }
}

impl<S> ProviderIdentityGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .validate_identity(api_key)
                    .await
                    .map_err(provider_setup_error_from_client_error),
            }
        })
    }
}

impl<S> ProviderPlacementOptionsGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn fetch_placement_options<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let api_key = self
                .secrets
                .read_api_key(provider_id)?
                .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

            let provider_inventory = match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .fetch_inventory(&api_key)
                    .await
                    .map_err(error_from_client_error),
            }?;
            let placement_capabilities = match provider_id {
                GpuCloudProviderId::Runpod => ProviderPlacementCapabilities::runpod(),
            };

            Ok(ProviderPlacementOptions {
                provider_inventory,
                placement_capabilities,
            })
        })
    }
}

impl<S> ProviderProvisioningGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .create_network_volume(
                        &api_key,
                        &RunPodCreateNetworkVolumeRequest {
                            name: provider_resource_name(&input.workspace_id, "volume"),
                            data_center_id: input.datacenter_id,
                            size: bytes_to_gib(input.size_bytes),
                        },
                    )
                    .await
                    .map(runpod_network_volume_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn get_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .get_network_volume(&api_key, volume_id)
                    .await
                    .map(runpod_network_volume_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .delete_network_volume(&api_key, volume_id)
                    .await
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .create_pod(
                        &api_key,
                        &RunPodCreatePodRequest {
                            name: provider_resource_name(&input.workspace_id, "provisioner"),
                            image_name: input.provisioner_worker_image_ref,
                            gpu_type_ids: vec![input.selected_gpu_id],
                            data_center_ids: vec![input.datacenter_id],
                            network_volume_id: input.network_volume_id,
                            volume_mount_path: input.mount_path,
                            env: HashMap::from([(
                                "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
                                input.bearer_token.expose_secret().to_string(),
                            )]),
                            ports: vec![format!("{}/http", input.provisioner_worker_port)],
                        },
                    )
                    .await
                    .map(runpod_pod_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn get_provisioning_pod<'a>(
        &'a self,
        input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .get_pod_with_context(
                        &api_key,
                        &input.provider_resource_id,
                        &input.datacenter_id,
                        &input.selected_gpu_id,
                    )
                    .await
                    .map(runpod_pod_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn discover_provisioning_pods<'a>(
        &'a self,
        input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .find_pods_by_name_and_volume(
                        &api_key,
                        &provider_resource_name(&input.workspace_id, "provisioner"),
                        &input.network_volume_id,
                        &input.datacenter_id,
                        &input.selected_gpu_id,
                    )
                    .await
                    .map(|observations| {
                        observations
                            .into_iter()
                            .map(runpod_pod_observation)
                            .collect()
                    })
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .delete_pod(&api_key, pod_id)
                    .await
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .create_template(
                        &api_key,
                        &RunPodCreateTemplateRequest {
                            name: provider_resource_name(&input.workspace_id, "endpoint-template"),
                            image_name: input.endpoint_worker_image_ref,
                            container_disk_in_gb: 10,
                            env: HashMap::new(),
                            is_public: false,
                            is_serverless: true,
                            ports: vec![format!("{}/http", input.endpoint_worker_port)],
                            readme: String::new(),
                            volume_mount_path: input.mount_path,
                        },
                    )
                    .await
                    .map(runpod_template_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn get_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .get_template(&api_key, template_id)
                    .await
                    .map(runpod_template_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .delete_template(&api_key, template_id)
                    .await
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&input.gpu_cloud_provider_id)?;
            match input.gpu_cloud_provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .create_endpoint(
                        &api_key,
                        &RunPodCreateEndpointRequest {
                            name: provider_resource_name(&input.workspace_id, "endpoint"),
                            template_id: input.template_id,
                            gpu_type_ids: vec![input.selected_gpu_id],
                            network_volume_id: input.network_volume_id,
                            data_center_ids: vec![input.datacenter_id],
                            workers_min: 0,
                            workers_max: 1,
                            scaler_type: "REQUEST_COUNT".to_string(),
                            scaler_value: 1,
                            idle_timeout: input.endpoint_keep_alive_seconds,
                        },
                    )
                    .await
                    .map(runpod_endpoint_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn get_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .get_endpoint(&api_key, endpoint_id)
                    .await
                    .map(runpod_endpoint_observation)
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = self.provisioning_api_key(&provider_id)?;
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .delete_endpoint(&api_key, endpoint_id)
                    .await
                    .map_err(provisioning_error_from_client_error),
            }
        })
    }
}

impl<S> ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn provisioning_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<ProviderApiKey, WorkspaceProvisioningError> {
        self.secrets
            .read_api_key(provider_id)
            .map_err(WorkspaceProvisioningError::from)?
            .ok_or(WorkspaceProvisioningError::ProviderSetupIncomplete)
    }
}

fn provider_setup_error_from_client_error(error: ProviderClientError) -> ProviderSetupError {
    match error {
        ProviderClientError::Unauthorized => ProviderSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable | ProviderClientError::RateLimited => {
            ProviderSetupError::ProviderApiUnavailable
        }
        ProviderClientError::RequestRejected
        | ProviderClientError::ResponseInvalid
        | ProviderClientError::NotFound
        | ProviderClientError::Conflict
        | ProviderClientError::Indeterminate => ProviderSetupError::ProviderIdentityResponseInvalid,
    }
}

fn error_from_client_error(error: ProviderClientError) -> WorkspaceSetupError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => WorkspaceSetupError::ProviderApiUnavailable,
        ProviderClientError::RateLimited => WorkspaceSetupError::ProviderRateLimited,
        ProviderClientError::RequestRejected => WorkspaceSetupError::ProviderRequestRejected,
        ProviderClientError::ResponseInvalid
        | ProviderClientError::NotFound
        | ProviderClientError::Conflict
        | ProviderClientError::Indeterminate => WorkspaceSetupError::ProviderResponseInvalid,
    }
}

fn provisioning_error_from_client_error(error: ProviderClientError) -> WorkspaceProvisioningError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceProvisioningError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => WorkspaceProvisioningError::ProviderApiUnavailable,
        ProviderClientError::RateLimited => WorkspaceProvisioningError::ProviderRateLimited,
        ProviderClientError::RequestRejected => WorkspaceProvisioningError::ProviderRequestRejected,
        ProviderClientError::ResponseInvalid => WorkspaceProvisioningError::ProviderResponseInvalid,
        ProviderClientError::NotFound => WorkspaceProvisioningError::ProviderResourceNotFound,
        ProviderClientError::Conflict => WorkspaceProvisioningError::ProviderOperationConflict,
        ProviderClientError::Indeterminate => {
            WorkspaceProvisioningError::ProviderOperationIndeterminate
        }
    }
}

fn runpod_network_volume_observation(
    observation: RunPodNetworkVolumeObservation,
) -> NetworkVolumeObservation {
    NetworkVolumeObservation {
        provider_resource_id: observation.id,
        datacenter_id: observation.data_center_id,
        provisioned_size_bytes: observation.size_gb * GIB_BYTES,
        provider_resource_status: observation.status,
        mount_path: RUNPOD_VOLUME_MOUNT_PATH.to_string(),
    }
}

fn runpod_pod_observation(observation: RunPodPodObservation) -> ProvisioningPodObservation {
    ProvisioningPodObservation {
        provider_resource_id: observation.id,
        datacenter_id: observation.data_center_id,
        selected_gpu_id: observation.selected_gpu_id,
        provider_resource_status: observation.status,
        provisioner_status_url: observation.provisioner_status_url,
    }
}

fn runpod_template_observation(
    observation: RunPodTemplateObservation,
) -> EndpointTemplateObservation {
    EndpointTemplateObservation {
        template_id: observation.id,
        endpoint_worker_image_ref: observation.image_name,
        mount_path: observation.volume_mount_path,
        provider_resource_status: observation.status,
    }
}

fn runpod_endpoint_observation(
    observation: RunPodEndpointObservation,
) -> ServerlessEndpointObservation {
    ServerlessEndpointObservation {
        provider_resource_id: observation.id,
        datacenter_id: observation.data_center_id,
        selected_gpu_id: observation.selected_gpu_id,
        provider_resource_status: observation.status,
        endpoint_invoke_url: observation.endpoint_invoke_url,
    }
}

fn bytes_to_gib(bytes: u64) -> u64 {
    bytes.div_ceil(GIB_BYTES)
}

fn provider_resource_name(workspace_id: &str, suffix: &str) -> String {
    format!("luma-forge-{workspace_id}-{suffix}")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
