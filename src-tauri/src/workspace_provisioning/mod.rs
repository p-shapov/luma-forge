use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    domain::{
        placement::PlacementPlan,
        provider_setup::GpuCloudProviderId,
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
            ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot,
            Workspace, WorkspaceLifecycleState,
        },
    },
    provisioner_worker::{
        progress_from_worker_status, ProvisionerWorkerError, ProvisionerWorkerJobStatus,
        ProvisionerWorkerStartRequest, ProvisionerWorkerStatus,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

pub mod error;

pub use error::WorkspaceProvisioningError;

pub trait ProviderProvisioningGateway: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

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
    >;

    fn delete_network_volume<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn get_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn delete_provisioning_pod<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

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
    >;

    fn delete_endpoint_template<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

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
    >;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>>;
}

#[derive(Debug, Clone)]
pub struct CreateNetworkVolumeInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub datacenter_id: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct NetworkVolumeObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provisioned_size_bytes: u64,
    pub provider_resource_status: ProviderResourceStatus,
    pub mount_path: String,
}

#[derive(Debug, Clone)]
pub struct CreateProvisioningPodInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub provisioner_worker_image_ref: String,
    pub provisioner_worker_port: u16,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub mount_path: String,
    pub bearer_token: ProvisionerWorkerBearerToken,
}

#[derive(Debug, Clone)]
pub struct ProvisioningPodObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioner_status_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateEndpointTemplateInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub endpoint_worker_image_ref: String,
    pub endpoint_worker_port: u16,
    pub mount_path: String,
}

#[derive(Debug, Clone)]
pub struct EndpointTemplateObservation {
    pub template_id: String,
    pub endpoint_worker_image_ref: String,
    pub mount_path: String,
    pub provider_resource_status: ProviderResourceStatus,
}

#[derive(Debug, Clone)]
pub struct CreateServerlessEndpointInput {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub workspace_id: String,
    pub template_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub network_volume_id: String,
    pub endpoint_keep_alive_seconds: u32,
}

#[derive(Debug, Clone)]
pub struct ServerlessEndpointObservation {
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub selected_gpu_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub endpoint_invoke_url: String,
}

#[derive(Debug, Clone)]
pub struct WorkspaceProvisioningResult {
    pub workspace: Workspace,
    pub progress: crate::domain::workspace::WorkspaceProvisioningProgress,
}

pub trait ProvisionerWorkerGateway: Send + Sync {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;

    fn cancel<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    >;
}

impl ProvisionerWorkerGateway for crate::provisioner_worker::ProvisionerWorkerHttpGateway {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.start(provisioner_status_url, token, request)
                .await
                .map_err(worker_error)
        })
    }

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.status(provisioner_status_url, token)
                .await
                .map_err(worker_error)
        })
    }

    fn cancel<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.cancel(provisioner_status_url, token)
                .await
                .map_err(worker_error)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceProvisioningConfig {
    pub provisioner_worker_image_ref: String,
    pub provisioner_worker_port: u16,
    pub runpod_endpoint_worker_image_ref: String,
    pub runpod_endpoint_worker_port: u16,
    pub volume_mount_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceProvisioningCoordinator {
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl WorkspaceProvisioningCoordinator {
    fn try_enter(&self, workspace_id: &str) -> Option<WorkspaceProvisioningGuard> {
        let mut active = self
            .active_workspace_ids
            .lock()
            .expect("workspace provisioning coordinator lock");
        if !active.insert(workspace_id.to_string()) {
            return None;
        }
        Some(WorkspaceProvisioningGuard {
            workspace_id: workspace_id.to_string(),
            active_workspace_ids: self.active_workspace_ids.clone(),
        })
    }
}

struct WorkspaceProvisioningGuard {
    workspace_id: String,
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl Drop for WorkspaceProvisioningGuard {
    fn drop(&mut self) {
        self.active_workspace_ids
            .lock()
            .expect("workspace provisioning coordinator lock")
            .remove(&self.workspace_id);
    }
}

pub struct WorkspaceProvisioningService<S, P, W, R> {
    secrets: S,
    providers: P,
    workspace_catalog: W,
    workers: R,
    coordinator: WorkspaceProvisioningCoordinator,
    config: WorkspaceProvisioningConfig,
}

impl<S, P, W, R> WorkspaceProvisioningService<S, P, W, R> {
    pub fn new(
        secrets: S,
        providers: P,
        workspace_catalog: W,
        workers: R,
        coordinator: WorkspaceProvisioningCoordinator,
        config: WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            providers,
            workspace_catalog,
            workers,
            coordinator,
            config,
        }
    }
}

impl<S, P, W, R> WorkspaceProvisioningService<S, P, W, R>
where
    S: SecretStore,
    P: ProviderProvisioningGateway,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    pub async fn initiate(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let mut workspace = self.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Draft {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }
        self.secrets
            .read_api_key(&workspace.gpu_cloud_provider_id)
            .map_err(WorkspaceProvisioningError::from)?
            .ok_or(WorkspaceProvisioningError::ProviderSetupIncomplete)?;

        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        let workspace = self.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }

    pub async fn sync(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Ok(result(self.workspace(workspace_id).await?));
        };

        let mut workspace = self.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Ok(result(workspace));
        }

        if workspace.persistent_storage_volume_snapshot.is_none() {
            let PlacementPlan::Runpod {
                selected_datacenter_id,
                persistent_storage_volume_size_bytes,
                ..
            } = &workspace.placement_plan;
            let observation = match self
                .providers
                .create_network_volume(CreateNetworkVolumeInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    datacenter_id: selected_datacenter_id.clone(),
                    size_bytes: *persistent_storage_volume_size_bytes,
                })
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderOperationIndeterminate) => {
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                    let workspace = self.update_workspace(&workspace).await?;
                    return Ok(result(workspace));
                }
                Err(error) => return Err(error),
            };
            workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                provider_resource_id: observation.provider_resource_id,
                datacenter_id: observation.datacenter_id,
                provider_resource_status: observation.provider_resource_status,
                provisioned_size_bytes: observation.provisioned_size_bytes,
                mount_path: observation.mount_path,
            });
            if workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                })
            {
                workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
            }
            let workspace = self.update_workspace(&workspace).await?;
            return Ok(result(workspace));
        }

        if let Some(snapshot) = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        {
            let observation = self
                .providers
                .get_network_volume(
                    workspace.gpu_cloud_provider_id,
                    &snapshot.provider_resource_id,
                )
                .await?;
            workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                provider_resource_id: observation.provider_resource_id,
                datacenter_id: observation.datacenter_id,
                provider_resource_status: observation.provider_resource_status,
                provisioned_size_bytes: observation.provisioned_size_bytes,
                mount_path: observation.mount_path,
            });
            if workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                })
            {
                workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
            }
            let workspace = self.update_workspace(&workspace).await?;
            return Ok(result(workspace));
        }

        if workspace.environment_prepared_at.is_none()
            && workspace.active_provisioning_pod_snapshot.is_none()
            && workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot.provider_resource_status == ProviderResourceStatus::Ready
                })
        {
            let volume = workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .expect("volume checked above");
            let PlacementPlan::Runpod {
                selected_datacenter_id,
                selected_gpu_id,
                ..
            } = &workspace.placement_plan;
            let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
                .map_err(|_| WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid)?;
            self.secrets
                .write_provisioner_worker_token(&workspace.id, &token)
                .map_err(WorkspaceProvisioningError::from)?;
            let observation = self
                .providers
                .create_provisioning_pod(CreateProvisioningPodInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    provisioner_worker_image_ref: self.config.provisioner_worker_image_ref.clone(),
                    provisioner_worker_port: self.config.provisioner_worker_port,
                    datacenter_id: selected_datacenter_id.clone(),
                    selected_gpu_id: selected_gpu_id.clone(),
                    network_volume_id: volume.provider_resource_id.clone(),
                    mount_path: self.config.volume_mount_path.clone(),
                    bearer_token: token,
                })
                .await?;
            workspace.active_provisioning_pod_snapshot = Some(ProvisioningPodSnapshot {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                provider_resource_id: observation.provider_resource_id,
                datacenter_id: observation.datacenter_id,
                provider_resource_status: observation.provider_resource_status,
                selected_gpu_id: observation.selected_gpu_id,
                provisioner_status_url: observation
                    .provisioner_status_url
                    .ok_or(WorkspaceProvisioningError::ProviderResponseInvalid)?,
            });
            let workspace = self.update_workspace(&workspace).await?;
            return Ok(result(workspace));
        }

        if workspace.environment_prepared_at.is_none() {
            if let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() {
                let observation = self
                    .providers
                    .get_provisioning_pod(
                        workspace.gpu_cloud_provider_id,
                        &active_pod.provider_resource_id,
                    )
                    .await?;
                let observed_pod = ProvisioningPodSnapshot {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    provider_resource_id: observation.provider_resource_id,
                    datacenter_id: observation.datacenter_id,
                    provider_resource_status: observation.provider_resource_status,
                    selected_gpu_id: observation.selected_gpu_id,
                    provisioner_status_url: observation
                        .provisioner_status_url
                        .unwrap_or_else(|| active_pod.provisioner_status_url.clone()),
                };
                if is_terminal_provider_resource_status(&observed_pod.provider_resource_status) {
                    workspace.active_provisioning_pod_snapshot = Some(observed_pod);
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                    let workspace = self.update_workspace(&workspace).await?;
                    return Ok(result(workspace));
                }
                if observed_pod != active_pod {
                    workspace.active_provisioning_pod_snapshot = Some(observed_pod);
                    let workspace = self.update_workspace(&workspace).await?;
                    return Ok(result(workspace));
                }
                if active_pod.provider_resource_status != ProviderResourceStatus::Running {
                    return Ok(result(workspace));
                }

                let token = match self
                    .secrets
                    .read_provisioner_worker_token(&workspace.id)
                    .map_err(WorkspaceProvisioningError::from)?
                {
                    Some(token) => token,
                    None => {
                        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                        let workspace = self.update_workspace(&workspace).await?;
                        return Ok(result(workspace));
                    }
                };
                let worker_status = match self
                    .workers
                    .status(&active_pod.provisioner_status_url, &token)
                    .await
                {
                    Ok(status) if status.status == ProvisionerWorkerJobStatus::Idle => {
                        self.workers
                            .start(
                                &active_pod.provisioner_status_url,
                                &token,
                                &ProvisionerWorkerStartRequest {
                                    workspace_id: workspace.id.clone(),
                                    workflow_preset: workspace
                                        .placement_plan
                                        .selected_workflow_preset()
                                        .clone(),
                                },
                            )
                            .await?
                    }
                    Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
                        workspace.environment_prepared_at = Some(now_rfc3339()?);
                        let workspace = self.update_workspace(&workspace).await?;
                        return Ok(result(workspace));
                    }
                    Ok(status) => status,
                    Err(error) => {
                        return self
                            .fail_workspace_after_worker_error(workspace, error)
                            .await;
                    }
                };
                return Ok(WorkspaceProvisioningResult {
                    workspace,
                    progress: progress_from_worker_status(&worker_status),
                });
            }
        }

        if workspace.environment_prepared_at.is_some() {
            if let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() {
                match self
                    .providers
                    .delete_provisioning_pod(
                        workspace.gpu_cloud_provider_id,
                        &active_pod.provider_resource_id,
                    )
                    .await
                {
                    Ok(()) | Err(WorkspaceProvisioningError::ProviderResourceNotFound) => {}
                    Err(error) => return Err(error),
                }
                let mut terminal_pod = active_pod;
                terminal_pod.provider_resource_status = ProviderResourceStatus::Terminated;
                workspace.last_provisioning_pod_snapshot = Some(terminal_pod);
                workspace.active_provisioning_pod_snapshot = None;
                self.secrets
                    .delete_provisioner_worker_token(&workspace.id)
                    .map_err(WorkspaceProvisioningError::from)?;
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }

            let template_snapshot = runpod_template_snapshot(&workspace);
            if template_snapshot.is_none() {
                let observation = self
                    .providers
                    .create_endpoint_template(CreateEndpointTemplateInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                        endpoint_worker_image_ref: self
                            .config
                            .runpod_endpoint_worker_image_ref
                            .clone(),
                        endpoint_worker_port: self.config.runpod_endpoint_worker_port,
                        mount_path: self.config.volume_mount_path.clone(),
                    })
                    .await?;
                workspace.provider_provisioning_snapshot =
                    Some(ProviderProvisioningSnapshot::Runpod {
                        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                            template_id: observation.template_id,
                            endpoint_worker_image_ref: observation.endpoint_worker_image_ref,
                            mount_path: observation.mount_path,
                            provider_resource_status: observation.provider_resource_status,
                        }),
                    });
                if runpod_template_snapshot(&workspace)
                    .as_ref()
                    .is_some_and(|snapshot| {
                        is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                    })
                {
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                }
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }

            if let Some(template) = template_snapshot.filter(|snapshot| {
                snapshot.provider_resource_status != ProviderResourceStatus::Ready
            }) {
                let observation = self
                    .providers
                    .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
                    .await?;
                workspace.provider_provisioning_snapshot =
                    Some(ProviderProvisioningSnapshot::Runpod {
                        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
                            template_id: observation.template_id,
                            endpoint_worker_image_ref: observation.endpoint_worker_image_ref,
                            mount_path: observation.mount_path,
                            provider_resource_status: observation.provider_resource_status,
                        }),
                    });
                if runpod_template_snapshot(&workspace)
                    .as_ref()
                    .is_some_and(|snapshot| {
                        is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                    })
                {
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                }
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }

            if workspace.serverless_endpoint_snapshot.is_none() {
                let volume = workspace
                    .persistent_storage_volume_snapshot
                    .as_ref()
                    .ok_or(WorkspaceProvisioningError::ProviderResponseInvalid)?;
                let template = runpod_template_snapshot(&workspace)
                    .ok_or(WorkspaceProvisioningError::ProviderResponseInvalid)?;
                let PlacementPlan::Runpod {
                    selected_datacenter_id,
                    selected_gpu_id,
                    endpoint_keep_alive_seconds,
                    ..
                } = &workspace.placement_plan;
                let observation = self
                    .providers
                    .create_serverless_endpoint(CreateServerlessEndpointInput {
                        gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                        workspace_id: workspace.id.clone(),
                        template_id: template.template_id,
                        datacenter_id: selected_datacenter_id.clone(),
                        selected_gpu_id: selected_gpu_id.clone(),
                        network_volume_id: volume.provider_resource_id.clone(),
                        endpoint_keep_alive_seconds: *endpoint_keep_alive_seconds,
                    })
                    .await?;
                workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    provider_resource_id: observation.provider_resource_id,
                    datacenter_id: observation.datacenter_id,
                    provider_resource_status: observation.provider_resource_status,
                    selected_gpu_id: observation.selected_gpu_id,
                    endpoint_invoke_url: observation.endpoint_invoke_url,
                });
                if workspace
                    .serverless_endpoint_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| {
                        is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                    })
                {
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                }
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }

            if let Some(endpoint) =
                workspace
                    .serverless_endpoint_snapshot
                    .as_ref()
                    .filter(|snapshot| {
                        snapshot.provider_resource_status != ProviderResourceStatus::Ready
                    })
            {
                let observation = self
                    .providers
                    .get_serverless_endpoint(
                        workspace.gpu_cloud_provider_id,
                        &endpoint.provider_resource_id,
                    )
                    .await?;
                workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    provider_resource_id: observation.provider_resource_id,
                    datacenter_id: observation.datacenter_id,
                    provider_resource_status: observation.provider_resource_status,
                    selected_gpu_id: observation.selected_gpu_id,
                    endpoint_invoke_url: observation.endpoint_invoke_url,
                });
                if workspace
                    .serverless_endpoint_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| {
                        is_terminal_provider_resource_status(&snapshot.provider_resource_status)
                    })
                {
                    workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
                }
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }

            if is_workspace_ready(&workspace) {
                workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
                let workspace = self.update_workspace(&workspace).await?;
                return Ok(result(workspace));
            }
        }

        Ok(result(workspace))
    }

    pub async fn cancel(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Ok(result(self.workspace(workspace_id).await?));
        };

        let mut workspace = self.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }

        match crate::workspace_resource_cleanup::cleanup_known_resources(
            &self.secrets,
            &self.providers,
            &self.workers,
            &workspace,
        )
        .await
        {
            Ok(()) => {
                workspace.lifecycle_state = WorkspaceLifecycleState::Draft;
                workspace.persistent_storage_volume_snapshot = None;
                workspace.active_provisioning_pod_snapshot = None;
                workspace.serverless_endpoint_snapshot = None;
                workspace.last_provisioning_pod_snapshot = None;
                workspace.provider_provisioning_snapshot = None;
                workspace.environment_prepared_at = None;
            }
            Err(_) => {
                workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
            }
        }

        let workspace = self.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }

    async fn fail_workspace_after_worker_error(
        &self,
        mut workspace: Workspace,
        error: WorkspaceProvisioningError,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        if error.is_terminal_worker_failure() {
            workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
            let workspace = self.update_workspace(&workspace).await?;
            Ok(result(workspace))
        } else {
            Err(error)
        }
    }

    async fn workspace(&self, workspace_id: &str) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(catalog_error)?
            .ok_or(WorkspaceProvisioningError::WorkspaceNotFound)
    }

    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(catalog_error)
    }
}

fn result(workspace: Workspace) -> WorkspaceProvisioningResult {
    let progress = progress_for_workspace(&workspace);
    WorkspaceProvisioningResult {
        workspace,
        progress,
    }
}

fn progress_for_workspace(
    workspace: &Workspace,
) -> crate::domain::workspace::WorkspaceProvisioningProgress {
    use crate::domain::workspace::{WorkspaceProvisioningPhase, WorkspaceProvisioningStatus};

    match workspace.lifecycle_state {
        WorkspaceLifecycleState::Draft => crate::domain::workspace::WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Idle,
            phase: WorkspaceProvisioningPhase::NotStarted,
            percent: Some(0),
            message: None,
        },
        WorkspaceLifecycleState::Provisioning => {
            crate::domain::workspace::WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Running,
                phase: if workspace.persistent_storage_volume_snapshot.is_none() {
                    WorkspaceProvisioningPhase::CreatingVolume
                } else if workspace.active_provisioning_pod_snapshot.is_none()
                    && workspace.environment_prepared_at.is_none()
                {
                    WorkspaceProvisioningPhase::StartingProvisioningPod
                } else if workspace.environment_prepared_at.is_none()
                    || workspace.active_provisioning_pod_snapshot.is_some()
                {
                    WorkspaceProvisioningPhase::PreparingEnvironment
                } else if runpod_template_snapshot(workspace).is_none() {
                    WorkspaceProvisioningPhase::CreatingEndpointTemplate
                } else if workspace.serverless_endpoint_snapshot.is_none() {
                    WorkspaceProvisioningPhase::CreatingEndpoint
                } else {
                    WorkspaceProvisioningPhase::ValidatingReadiness
                },
                percent: None,
                message: None,
            }
        }
        WorkspaceLifecycleState::Ready => crate::domain::workspace::WorkspaceProvisioningProgress {
            status: WorkspaceProvisioningStatus::Completed,
            phase: WorkspaceProvisioningPhase::Completed,
            percent: Some(100),
            message: None,
        },
        WorkspaceLifecycleState::Failed => {
            crate::domain::workspace::WorkspaceProvisioningProgress {
                status: WorkspaceProvisioningStatus::Failed,
                phase: WorkspaceProvisioningPhase::Failed,
                percent: None,
                message: None,
            }
        }
    }
}

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}

fn worker_error(error: ProvisionerWorkerError) -> WorkspaceProvisioningError {
    match error {
        ProvisionerWorkerError::Unauthorized => {
            WorkspaceProvisioningError::ProvisionerWorkerUnauthorized
        }
        ProvisionerWorkerError::Conflict => WorkspaceProvisioningError::ProvisionerWorkerConflict,
        ProvisionerWorkerError::Unreachable => {
            WorkspaceProvisioningError::ProvisionerWorkerUnavailable
        }
        ProvisionerWorkerError::InvalidPayload => {
            WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid
        }
        ProvisionerWorkerError::TerminalFailure { diagnostic } => {
            WorkspaceProvisioningError::ProvisionerWorkerFailed { diagnostic }
        }
    }
}

fn now_rfc3339() -> Result<String, WorkspaceProvisioningError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| WorkspaceProvisioningError::ProviderResponseInvalid)
}

fn runpod_template_snapshot(workspace: &Workspace) -> Option<RunPodEndpointTemplateSnapshot> {
    match &workspace.provider_provisioning_snapshot {
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot,
        }) => endpoint_template_snapshot.clone(),
        None => None,
    }
}

fn is_workspace_ready(workspace: &Workspace) -> bool {
    workspace.environment_prepared_at.is_some()
        && workspace.active_provisioning_pod_snapshot.is_none()
        && workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && runpod_template_snapshot(workspace)
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
        && workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.provider_resource_status == ProviderResourceStatus::Ready
            })
}

fn is_terminal_provider_resource_status(status: &ProviderResourceStatus) -> bool {
    matches!(
        status,
        ProviderResourceStatus::Failed
            | ProviderResourceStatus::Terminated
            | ProviderResourceStatus::Unknown
    )
}

#[cfg(test)]
mod tests;
