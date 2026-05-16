use crate::{
    domain::{
        placement::PlacementPlan,
        workspace::{
            ProviderResourceStatus, Workspace, WorkspaceLifecycleState, WorkspaceProvisioningPhase,
            WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
        },
    },
    provisioner_worker::{
        progress_from_worker_status, ProvisionerWorkerJobStatus, ProvisionerWorkerStartRequest,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    contracts::{
        CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput, DiscoverNetworkVolumesInput,
        DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput,
        ObserveProvisioningPodInput, WorkspaceProvisioningResult,
    },
    coordinator::WorkspaceProvisioningCoordinator,
    failure,
    gateways::{ProviderProvisioningGateway, ProvisionerWorkerGateway},
    progress::result,
    snapshots::{
        created_provisioning_pod_snapshot, is_terminal_provider_resource_status,
        is_workspace_ready, observed_provisioning_pod_snapshot, persistent_storage_volume_snapshot,
        runpod_template_provisioning_snapshot, runpod_template_snapshot,
        serverless_endpoint_snapshot,
    },
    WorkspaceProvisioningError,
};

type SyncStepResult = Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceProvisioningConfig {
    pub provisioner_worker_image_ref: String,
    pub provisioner_worker_port: u16,
    pub runpod_endpoint_worker_image_ref: String,
    pub runpod_endpoint_worker_port: u16,
    pub volume_mount_path: String,
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
        workspace.last_provisioning_failure = None;
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

        if let Some(result) = self.sync_network_volume(&mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = self.sync_provisioning_pod(&mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = self.drive_provisioner_worker(&mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = self.finish_provisioning_pod(&mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = self.sync_endpoint_template(&mut workspace).await? {
            return Ok(result);
        }
        if let Some(result) = self.sync_serverless_endpoint(&mut workspace).await? {
            return Ok(result);
        }

        Ok(result(workspace))
    }

    pub async fn cancel(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceProvisioningResult, WorkspaceProvisioningError> {
        let Some(_guard) = self.coordinator.try_enter(workspace_id) else {
            return Err(WorkspaceProvisioningError::ProviderOperationConflict);
        };

        let mut workspace = self.workspace(workspace_id).await?;
        if workspace.lifecycle_state != WorkspaceLifecycleState::Provisioning {
            return Err(WorkspaceProvisioningError::InvalidWorkspaceLifecycle);
        }

        match crate::workspace_resource_cleanup::cleanup_known_resources(
            &self.secrets,
            &self.providers,
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
                workspace.last_provisioning_failure = None;
            }
            Err(_) => {
                failure::fail_workspace(&mut workspace, failure::cancellation_cleanup_failed());
            }
        }

        let workspace = self.update_workspace(&workspace).await?;
        Ok(result(workspace))
    }

    async fn sync_network_volume(&self, workspace: &mut Workspace) -> SyncStepResult {
        if workspace.persistent_storage_volume_snapshot.is_none() {
            let PlacementPlan::Runpod {
                selected_datacenter_id,
                persistent_storage_volume_size_bytes,
                ..
            } = &workspace.placement_plan;
            let selected_datacenter_id = selected_datacenter_id.clone();
            let persistent_storage_volume_size_bytes = *persistent_storage_volume_size_bytes;
            let discovered_volumes = self
                .providers
                .discover_network_volumes(DiscoverNetworkVolumesInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    datacenter_id: selected_datacenter_id.clone(),
                    size_bytes: persistent_storage_volume_size_bytes,
                })
                .await?;
            match discovered_volumes.as_slice() {
                [] => {}
                [observation] => {
                    workspace.persistent_storage_volume_snapshot = Some(
                        persistent_storage_volume_snapshot(workspace, observation.clone()),
                    );
                    self.fail_if_volume_status_is_terminal(workspace);
                    let workspace = self.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
                _ => {
                    return self
                        .fail_for_indeterminate_provider_operation(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingVolume,
                        )
                        .await;
                }
            }
            let observation = match self
                .providers
                .create_network_volume(CreateNetworkVolumeInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    datacenter_id: selected_datacenter_id,
                    size_bytes: persistent_storage_volume_size_bytes,
                })
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderOperationIndeterminate) => {
                    let discovered_volumes = self
                        .providers
                        .discover_network_volumes(DiscoverNetworkVolumesInput {
                            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                            workspace_id: workspace.id.clone(),
                            datacenter_id: workspace
                                .persistent_storage_volume_snapshot
                                .as_ref()
                                .map(|snapshot| snapshot.datacenter_id.clone())
                                .unwrap_or_else(|| match &workspace.placement_plan {
                                    PlacementPlan::Runpod {
                                        selected_datacenter_id,
                                        ..
                                    } => selected_datacenter_id.clone(),
                                }),
                            size_bytes: persistent_storage_volume_size_bytes,
                        })
                        .await?;
                    match discovered_volumes.as_slice() {
                        [observation] => {
                            workspace.persistent_storage_volume_snapshot = Some(
                                persistent_storage_volume_snapshot(workspace, observation.clone()),
                            );
                            self.fail_if_volume_status_is_terminal(workspace);
                            let workspace = self.update_workspace(workspace).await?;
                            return Ok(Some(result(workspace)));
                        }
                        _ => {
                            return self
                                .fail_for_indeterminate_provider_operation(
                                    workspace,
                                    WorkspaceProvisioningPhase::CreatingVolume,
                                )
                                .await;
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            workspace.persistent_storage_volume_snapshot =
                Some(persistent_storage_volume_snapshot(workspace, observation));
            self.fail_if_volume_status_is_terminal(workspace);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        let Some(volume_id) = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
            .map(|snapshot| snapshot.provider_resource_id.clone())
        else {
            return Ok(None);
        };

        let observation = match self
            .providers
            .get_network_volume(workspace.gpu_cloud_provider_id, &volume_id)
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceProvisioningError::ProviderResourceNotFound) => {
                return self
                    .fail_for_missing_provider_resource(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingVolume,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        workspace.persistent_storage_volume_snapshot =
            Some(persistent_storage_volume_snapshot(workspace, observation));
        self.fail_if_volume_status_is_terminal(workspace);
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    async fn sync_provisioning_pod(&self, workspace: &mut Workspace) -> SyncStepResult {
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
            let network_volume_id = volume.provider_resource_id.clone();
            let PlacementPlan::Runpod {
                selected_datacenter_id,
                selected_gpu_id,
                ..
            } = &workspace.placement_plan;
            let discovered_pods = self
                .providers
                .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    datacenter_id: selected_datacenter_id.clone(),
                    selected_gpu_id: selected_gpu_id.clone(),
                    network_volume_id: network_volume_id.clone(),
                })
                .await?;
            match discovered_pods.as_slice() {
                [] => {}
                [observation] => {
                    workspace.active_provisioning_pod_snapshot = Some(
                        created_provisioning_pod_snapshot(workspace, observation.clone())?,
                    );
                    let workspace = self.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
                _ => {
                    failure::fail_workspace(
                        workspace,
                        failure::indeterminate_provider_operation(
                            WorkspaceProvisioningPhase::StartingProvisioningPod,
                        ),
                    );
                    let workspace = self.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
            }
            let token = ProvisionerWorkerBearerToken::new(uuid::Uuid::new_v4().to_string())
                .map_err(|_| WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid)?;
            self.secrets
                .write_provisioner_worker_token(&workspace.id, &token)
                .map_err(WorkspaceProvisioningError::from)?;
            let observation = match self
                .providers
                .create_provisioning_pod(CreateProvisioningPodInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    provisioner_worker_image_ref: self.config.provisioner_worker_image_ref.clone(),
                    provisioner_worker_port: self.config.provisioner_worker_port,
                    datacenter_id: selected_datacenter_id.clone(),
                    selected_gpu_id: selected_gpu_id.clone(),
                    network_volume_id: network_volume_id.clone(),
                    mount_path: self.config.volume_mount_path.clone(),
                    bearer_token: token,
                })
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderOperationIndeterminate) => {
                    let discovered_pods = self
                        .providers
                        .discover_provisioning_pods(DiscoverProvisioningPodsInput {
                            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                            workspace_id: workspace.id.clone(),
                            datacenter_id: selected_datacenter_id.clone(),
                            selected_gpu_id: selected_gpu_id.clone(),
                            network_volume_id,
                        })
                        .await?;
                    match discovered_pods.as_slice() {
                        [observation] => {
                            workspace.active_provisioning_pod_snapshot = Some(
                                created_provisioning_pod_snapshot(workspace, observation.clone())?,
                            );
                            let workspace = self.update_workspace(workspace).await?;
                            return Ok(Some(result(workspace)));
                        }
                        _ => {
                            return self
                                .fail_for_indeterminate_provider_operation(
                                    workspace,
                                    WorkspaceProvisioningPhase::StartingProvisioningPod,
                                )
                                .await;
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            workspace.active_provisioning_pod_snapshot =
                Some(created_provisioning_pod_snapshot(workspace, observation)?);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        if workspace.environment_prepared_at.is_some() {
            return Ok(None);
        }

        let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
            return Ok(None);
        };

        let observation = match self
            .providers
            .get_provisioning_pod(ObserveProvisioningPodInput {
                gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                provider_resource_id: active_pod.provider_resource_id.clone(),
                datacenter_id: active_pod.datacenter_id.clone(),
                selected_gpu_id: active_pod.selected_gpu_id.clone(),
            })
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceProvisioningError::ProviderResourceNotFound) => {
                return self
                    .fail_for_missing_provider_resource(
                        workspace,
                        WorkspaceProvisioningPhase::StartingProvisioningPod,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        let observed_pod = observed_provisioning_pod_snapshot(workspace, &active_pod, observation);
        if is_terminal_provider_resource_status(&observed_pod.provider_resource_status) {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::StartingProvisioningPod,
                &observed_pod.provider_resource_status,
            );
            workspace.active_provisioning_pod_snapshot = Some(observed_pod);
            failure::fail_workspace(workspace, failure);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }
        if observed_pod != active_pod {
            workspace.active_provisioning_pod_snapshot = Some(observed_pod);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }
        if active_pod.provider_resource_status != ProviderResourceStatus::Running {
            return Ok(Some(result(workspace.clone())));
        }

        Ok(None)
    }

    async fn drive_provisioner_worker(&self, workspace: &mut Workspace) -> SyncStepResult {
        if workspace.environment_prepared_at.is_some() {
            return Ok(None);
        }

        let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
            return Ok(None);
        };

        if active_pod.provider_resource_status != ProviderResourceStatus::Running {
            return Ok(None);
        }

        let token = match self.secrets.read_provisioner_worker_token(&workspace.id) {
            Ok(Some(token)) => token,
            Ok(None) => {
                failure::fail_workspace(
                    workspace,
                    failure::worker_token_missing(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = self.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken) => {
                failure::fail_workspace(
                    workspace,
                    failure::worker_token_invalid(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = self.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(error) => return Err(WorkspaceProvisioningError::from(error)),
        };
        let worker_status = match self
            .workers
            .status(&active_pod.provisioner_status_url, &token)
            .await
        {
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Idle => {
                match self
                    .workers
                    .start(
                        &active_pod.provisioner_status_url,
                        &token,
                        &ProvisionerWorkerStartRequest {
                            job_id: workspace.id.clone(),
                            workflow_preset: workspace
                                .placement_plan
                                .selected_workflow_preset()
                                .clone(),
                        },
                    )
                    .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        return self.handle_worker_error(workspace.clone(), error).await;
                    }
                }
            }
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
                workspace.environment_prepared_at = Some(now_rfc3339()?);
                let workspace = self.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Ok(status) => status,
            Err(error) => {
                return self.handle_worker_error(workspace.clone(), error).await;
            }
        };
        Ok(Some(WorkspaceProvisioningResult {
            workspace: workspace.clone(),
            progress: progress_from_worker_status(&worker_status),
        }))
    }

    async fn finish_provisioning_pod(&self, workspace: &mut Workspace) -> SyncStepResult {
        if workspace.environment_prepared_at.is_none() {
            return Ok(None);
        }

        let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
            return Ok(None);
        };

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
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    async fn sync_endpoint_template(&self, workspace: &mut Workspace) -> SyncStepResult {
        if workspace.environment_prepared_at.is_none()
            || workspace.active_provisioning_pod_snapshot.is_some()
        {
            return Ok(None);
        }

        let template_snapshot = runpod_template_snapshot(workspace);
        if template_snapshot.is_none() {
            let discovered_templates = self
                .providers
                .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    endpoint_worker_image_ref: self.config.runpod_endpoint_worker_image_ref.clone(),
                    endpoint_worker_port: self.config.runpod_endpoint_worker_port,
                    mount_path: self.config.volume_mount_path.clone(),
                })
                .await?;
            match discovered_templates.as_slice() {
                [] => {}
                [observation] => {
                    workspace.provider_provisioning_snapshot =
                        Some(runpod_template_provisioning_snapshot(observation.clone()));
                    self.fail_if_template_status_is_terminal(workspace);
                    let workspace = self.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
                _ => {
                    return self
                        .fail_for_indeterminate_provider_operation(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                        )
                        .await;
                }
            }
            let observation = match self
                .providers
                .create_endpoint_template(CreateEndpointTemplateInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    endpoint_worker_image_ref: self.config.runpod_endpoint_worker_image_ref.clone(),
                    endpoint_worker_port: self.config.runpod_endpoint_worker_port,
                    mount_path: self.config.volume_mount_path.clone(),
                })
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderOperationIndeterminate) => {
                    let discovered_templates = self
                        .providers
                        .discover_endpoint_templates(DiscoverEndpointTemplatesInput {
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
                    match discovered_templates.as_slice() {
                        [observation] => {
                            workspace.provider_provisioning_snapshot =
                                Some(runpod_template_provisioning_snapshot(observation.clone()));
                            self.fail_if_template_status_is_terminal(workspace);
                            let workspace = self.update_workspace(workspace).await?;
                            return Ok(Some(result(workspace)));
                        }
                        _ => {
                            return self
                                .fail_for_indeterminate_provider_operation(
                                    workspace,
                                    WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                                )
                                .await;
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            workspace.provider_provisioning_snapshot =
                Some(runpod_template_provisioning_snapshot(observation));
            self.fail_if_template_status_is_terminal(workspace);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        let Some(template) = template_snapshot
            .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
        else {
            return Ok(None);
        };

        let observation = match self
            .providers
            .get_endpoint_template(workspace.gpu_cloud_provider_id, &template.template_id)
            .await
        {
            Ok(observation) => observation,
            Err(WorkspaceProvisioningError::ProviderResourceNotFound) => {
                return self
                    .fail_for_missing_provider_resource(
                        workspace,
                        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        workspace.provider_provisioning_snapshot =
            Some(runpod_template_provisioning_snapshot(observation));
        self.fail_if_template_status_is_terminal(workspace);
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    async fn sync_serverless_endpoint(&self, workspace: &mut Workspace) -> SyncStepResult {
        if workspace.environment_prepared_at.is_none()
            || workspace.active_provisioning_pod_snapshot.is_some()
        {
            return Ok(None);
        }

        if workspace.serverless_endpoint_snapshot.is_none() {
            let volume = workspace
                .persistent_storage_volume_snapshot
                .as_ref()
                .cloned();
            let Some(volume) = volume else {
                failure::fail_workspace(
                    workspace,
                    failure::missing_provider_resource(
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    ),
                );
                let workspace = self.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            };
            let Some(template) = runpod_template_snapshot(workspace) else {
                failure::fail_workspace(
                    workspace,
                    failure::readiness_validation_failed(
                        WorkspaceProvisioningPhase::CreatingEndpoint,
                    ),
                );
                let workspace = self.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            };
            let PlacementPlan::Runpod {
                selected_datacenter_id,
                selected_gpu_id,
                endpoint_keep_alive_seconds,
                ..
            } = &workspace.placement_plan;
            let selected_datacenter_id = selected_datacenter_id.clone();
            let selected_gpu_id = selected_gpu_id.clone();
            let endpoint_keep_alive_seconds = *endpoint_keep_alive_seconds;
            let discovered_endpoints = self
                .providers
                .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    template_id: template.template_id.clone(),
                    datacenter_id: selected_datacenter_id.clone(),
                    selected_gpu_id: selected_gpu_id.clone(),
                    network_volume_id: volume.provider_resource_id.clone(),
                    endpoint_keep_alive_seconds,
                })
                .await?;
            match discovered_endpoints.as_slice() {
                [] => {}
                [observation] => {
                    workspace.serverless_endpoint_snapshot =
                        Some(serverless_endpoint_snapshot(workspace, observation.clone()));
                    self.fail_if_endpoint_status_is_terminal(workspace);
                    let workspace = self.update_workspace(workspace).await?;
                    return Ok(Some(result(workspace)));
                }
                _ => {
                    return self
                        .fail_for_indeterminate_provider_operation(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingEndpoint,
                        )
                        .await;
                }
            }
            let observation = match self
                .providers
                .create_serverless_endpoint(CreateServerlessEndpointInput {
                    gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                    workspace_id: workspace.id.clone(),
                    template_id: template.template_id.clone(),
                    datacenter_id: selected_datacenter_id.clone(),
                    selected_gpu_id: selected_gpu_id.clone(),
                    network_volume_id: volume.provider_resource_id.clone(),
                    endpoint_keep_alive_seconds,
                })
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderOperationIndeterminate) => {
                    let discovered_endpoints = self
                        .providers
                        .discover_serverless_endpoints(DiscoverServerlessEndpointsInput {
                            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id,
                            workspace_id: workspace.id.clone(),
                            template_id: template.template_id,
                            datacenter_id: selected_datacenter_id,
                            selected_gpu_id,
                            network_volume_id: volume.provider_resource_id.clone(),
                            endpoint_keep_alive_seconds,
                        })
                        .await?;
                    match discovered_endpoints.as_slice() {
                        [observation] => {
                            workspace.serverless_endpoint_snapshot =
                                Some(serverless_endpoint_snapshot(workspace, observation.clone()));
                            self.fail_if_endpoint_status_is_terminal(workspace);
                            let workspace = self.update_workspace(workspace).await?;
                            return Ok(Some(result(workspace)));
                        }
                        _ => {
                            return self
                                .fail_for_indeterminate_provider_operation(
                                    workspace,
                                    WorkspaceProvisioningPhase::CreatingEndpoint,
                                )
                                .await;
                        }
                    }
                }
                Err(error) => return Err(error),
            };
            workspace.serverless_endpoint_snapshot =
                Some(serverless_endpoint_snapshot(workspace, observation));
            self.fail_if_endpoint_status_is_terminal(workspace);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        if let Some(endpoint_id) = workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.provider_resource_status != ProviderResourceStatus::Ready)
            .map(|snapshot| snapshot.provider_resource_id.clone())
        {
            let observation = match self
                .providers
                .get_serverless_endpoint(workspace.gpu_cloud_provider_id, &endpoint_id)
                .await
            {
                Ok(observation) => observation,
                Err(WorkspaceProvisioningError::ProviderResourceNotFound) => {
                    return self
                        .fail_for_missing_provider_resource(
                            workspace,
                            WorkspaceProvisioningPhase::CreatingEndpoint,
                        )
                        .await;
                }
                Err(error) => return Err(error),
            };
            workspace.serverless_endpoint_snapshot =
                Some(serverless_endpoint_snapshot(workspace, observation));
            self.fail_if_endpoint_status_is_terminal(workspace);
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        if is_workspace_ready(workspace) {
            workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
            workspace.last_provisioning_failure = None;
            let workspace = self.update_workspace(workspace).await?;
            return Ok(Some(result(workspace)));
        }

        Ok(None)
    }

    async fn handle_worker_error(
        &self,
        mut workspace: Workspace,
        error: WorkspaceProvisioningError,
    ) -> Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError> {
        if error == WorkspaceProvisioningError::ProvisionerWorkerUnavailable {
            return Ok(Some(WorkspaceProvisioningResult {
                workspace,
                progress: worker_readiness_progress(),
            }));
        }

        if let Some(failure) =
            failure::worker_failure(WorkspaceProvisioningPhase::PreparingEnvironment, &error)
        {
            failure::fail_workspace(&mut workspace, failure);
            let workspace = self.update_workspace(&workspace).await?;
            Ok(Some(result(workspace)))
        } else {
            Err(error)
        }
    }

    async fn fail_for_indeterminate_provider_operation(
        &self,
        workspace: &mut Workspace,
        phase: WorkspaceProvisioningPhase,
    ) -> SyncStepResult {
        failure::fail_workspace(workspace, failure::indeterminate_provider_operation(phase));
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
    }

    async fn fail_for_missing_provider_resource(
        &self,
        workspace: &mut Workspace,
        phase: WorkspaceProvisioningPhase,
    ) -> SyncStepResult {
        failure::fail_workspace(workspace, failure::missing_provider_resource(phase));
        let workspace = self.update_workspace(workspace).await?;
        Ok(Some(result(workspace)))
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

    fn fail_if_volume_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = workspace
            .persistent_storage_volume_snapshot
            .as_ref()
            .map(|snapshot| snapshot.provider_resource_status.clone())
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingVolume,
                &status,
            );
            failure::fail_workspace(workspace, failure);
        }
    }

    fn fail_if_template_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = runpod_template_snapshot(workspace)
            .map(|snapshot| snapshot.provider_resource_status)
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingEndpointTemplate,
                &status,
            );
            failure::fail_workspace(workspace, failure);
        }
    }

    fn fail_if_endpoint_status_is_terminal(&self, workspace: &mut Workspace) {
        if let Some(status) = workspace
            .serverless_endpoint_snapshot
            .as_ref()
            .map(|snapshot| snapshot.provider_resource_status.clone())
            .filter(is_terminal_provider_resource_status)
        {
            let failure = failure::provider_resource_failure(
                WorkspaceProvisioningPhase::CreatingEndpoint,
                &status,
            );
            failure::fail_workspace(workspace, failure);
        }
    }
}

fn worker_readiness_progress() -> WorkspaceProvisioningProgress {
    WorkspaceProvisioningProgress {
        status: WorkspaceProvisioningStatus::Running,
        phase: WorkspaceProvisioningPhase::PreparingEnvironment,
        percent: None,
        failure: None,
    }
}

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}

fn now_rfc3339() -> Result<String, WorkspaceProvisioningError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| WorkspaceProvisioningError::ProviderResponseInvalid)
}
