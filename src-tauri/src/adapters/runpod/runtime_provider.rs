use std::{collections::HashMap, time::Duration};

use secrecy::SecretString;
use tokio::time::Instant;

use crate::{
    application::runtimes::runpod::{
        CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodPlacement,
        RunpodPlacementDatacenter, RunpodPlacementGpu, RunpodRuntimeProvider,
        RunpodRuntimeProviderError, StartProvisionerPod, RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
    },
    providers::{
        runpod::{
            CreateEndpointRequest, CreateNetworkVolumeRequest, CreatePodRequest,
            CreateTemplateRequest, DeleteEndpointRequest, DeleteNetworkVolumeRequest,
            DeletePodRequest, DeleteTemplateRequest, PlacementRequest, PlacementResponse,
            ProvisionerStatusRequest, RunpodProvider,
        },
        NetworkError,
    },
};

const ENDPOINT_WORKERS_MIN: i64 = 0;
const ENDPOINT_WORKERS_MAX: i64 = 1;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const NOT_FOUND_TIMEOUT: Duration = Duration::from_secs(5 * 60);

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
    async fn placement(
        &self,
        #[diagnostic(redact)] api_key: &SecretString,
    ) -> Result<RunpodPlacement, RunpodRuntimeProviderError> {
        let response = self
            .provider
            .placement(PlacementRequest {
                credential: api_key.clone(),
            })
            .await
            .map_err(map_error)?;
        normalize_placement(response)
    }

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
        let mut not_found_deadline = None;

        loop {
            let response = match self
                .provider
                .provisioner_status(ProvisionerStatusRequest {
                    credential: api_key.clone(),
                    workspace_id: workspace_id.to_owned(),
                    pod_id: pod_id.to_owned(),
                })
                .await
            {
                Ok(response) => response,
                Err(NetworkError::NotFound) => {
                    let deadline = *not_found_deadline
                        .get_or_insert_with(|| Instant::now() + NOT_FOUND_TIMEOUT);
                    if Instant::now() >= deadline {
                        return Err(RunpodRuntimeProviderError::Unavailable);
                    }
                    tokio::time::sleep(POLL_INTERVAL).await;
                    continue;
                }
                Err(error) => return Err(map_error(error)),
            };
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

fn normalize_placement(
    response: PlacementResponse,
) -> Result<RunpodPlacement, RunpodRuntimeProviderError> {
    let gpus = response
        .gpu_types
        .ok_or(RunpodRuntimeProviderError::Unavailable)?
        .into_iter()
        .map(|gpu| {
            let gpu = gpu.ok_or(RunpodRuntimeProviderError::Unavailable)?;
            let id = gpu.id.ok_or(RunpodRuntimeProviderError::Unavailable)?;
            let gpu = RunpodPlacementGpu {
                id: id.clone(),
                name: gpu
                    .display_name
                    .ok_or(RunpodRuntimeProviderError::Unavailable)?,
                vram_gb: gpu
                    .memory_gb
                    .ok_or(RunpodRuntimeProviderError::Unavailable)?
                    .try_into()
                    .map_err(|_| RunpodRuntimeProviderError::Unavailable)?,
            };
            Ok((id, gpu))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;

    let datacenters = response
        .datacenters
        .ok_or(RunpodRuntimeProviderError::Unavailable)?
        .into_iter()
        .map(|datacenter| {
            let datacenter = datacenter.ok_or(RunpodRuntimeProviderError::Unavailable)?;
            let datacenter_gpus = datacenter
                .gpu_availability
                .ok_or(RunpodRuntimeProviderError::Unavailable)?
                .into_iter()
                .map(|availability| {
                    let id = availability
                        .ok_or(RunpodRuntimeProviderError::Unavailable)?
                        .gpu_type_id
                        .ok_or(RunpodRuntimeProviderError::Unavailable)?;
                    gpus.get(&id)
                        .cloned()
                        .ok_or(RunpodRuntimeProviderError::Unavailable)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(RunpodPlacementDatacenter {
                id: datacenter
                    .id
                    .ok_or(RunpodRuntimeProviderError::Unavailable)?,
                name: datacenter
                    .name
                    .ok_or(RunpodRuntimeProviderError::Unavailable)?,
                gpus: datacenter_gpus,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RunpodPlacement {
        max_volume_size_gb: RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
        datacenters,
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        application::runtimes::runpod::{
            RunpodPlacement, RunpodPlacementDatacenter, RunpodPlacementGpu,
            RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
        },
        providers::runpod::{
            PlacementDatacenter, PlacementGpuAvailability, PlacementGpuType, PlacementResponse,
        },
    };

    use super::{normalize_placement, RunpodRuntimeProviderError};

    #[test]
    fn placement_normalizes_complete_gpu_references_in_provider_order() {
        let response = PlacementResponse {
            gpu_types: Some(vec![
                Some(PlacementGpuType {
                    id: Some("NVIDIA RTX 4090".into()),
                    display_name: Some("RTX 4090".into()),
                    memory_gb: Some(24),
                }),
                Some(PlacementGpuType {
                    id: Some("NVIDIA A100".into()),
                    display_name: Some("A100".into()),
                    memory_gb: Some(80),
                }),
            ]),
            datacenters: Some(vec![
                Some(PlacementDatacenter {
                    id: Some("US-TX-1".into()),
                    name: Some("US Texas".into()),
                    gpu_availability: Some(vec![
                        Some(PlacementGpuAvailability {
                            gpu_type_id: Some("NVIDIA A100".into()),
                            available: Some(true),
                            stock_status: None,
                        }),
                        Some(PlacementGpuAvailability {
                            gpu_type_id: Some("NVIDIA RTX 4090".into()),
                            available: Some(false),
                            stock_status: None,
                        }),
                    ]),
                }),
                Some(PlacementDatacenter {
                    id: Some("EU-RO-1".into()),
                    name: Some("EU Romania".into()),
                    gpu_availability: Some(vec![Some(PlacementGpuAvailability {
                        gpu_type_id: Some("NVIDIA RTX 4090".into()),
                        available: Some(true),
                        stock_status: None,
                    })]),
                }),
            ]),
        };

        assert_eq!(
            normalize_placement(response),
            Ok(RunpodPlacement {
                max_volume_size_gb: RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
                datacenters: vec![
                    RunpodPlacementDatacenter {
                        id: "US-TX-1".into(),
                        name: "US Texas".into(),
                        gpus: vec![
                            RunpodPlacementGpu {
                                id: "NVIDIA A100".into(),
                                name: "A100".into(),
                                vram_gb: 80,
                            },
                            RunpodPlacementGpu {
                                id: "NVIDIA RTX 4090".into(),
                                name: "RTX 4090".into(),
                                vram_gb: 24,
                            },
                        ],
                    },
                    RunpodPlacementDatacenter {
                        id: "EU-RO-1".into(),
                        name: "EU Romania".into(),
                        gpus: vec![RunpodPlacementGpu {
                            id: "NVIDIA RTX 4090".into(),
                            name: "RTX 4090".into(),
                            vram_gb: 24,
                        }],
                    },
                ],
            })
        );
    }

    #[test]
    fn placement_rejects_unknown_availability_gpu_reference() {
        let response = PlacementResponse {
            gpu_types: Some(vec![Some(PlacementGpuType {
                id: Some("known".into()),
                display_name: Some("Known GPU".into()),
                memory_gb: Some(24),
            })]),
            datacenters: Some(vec![Some(PlacementDatacenter {
                id: Some("EU-RO-1".into()),
                name: Some("EU Romania".into()),
                gpu_availability: Some(vec![Some(PlacementGpuAvailability {
                    gpu_type_id: Some("unknown".into()),
                    available: Some(true),
                    stock_status: None,
                })]),
            })]),
        };

        assert_eq!(
            normalize_placement(response),
            Err(RunpodRuntimeProviderError::Unavailable)
        );
    }

    #[test]
    fn placement_rejects_malformed_gpu_data() {
        let response = PlacementResponse {
            gpu_types: Some(vec![Some(PlacementGpuType {
                id: Some("gpu".into()),
                display_name: Some("GPU".into()),
                memory_gb: Some(-1),
            })]),
            datacenters: Some(vec![]),
        };

        assert_eq!(
            normalize_placement(response),
            Err(RunpodRuntimeProviderError::Unavailable)
        );
    }
}
