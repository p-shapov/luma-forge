use crate::domain::{
    placement::{RemotePlacementOptions, RemotePlacementPlan},
    provider::GpuCloudProviderId,
    runtime_contract::RuntimeContractReference,
    workflow_preset::WorkflowPreset,
    workspace::{
        ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
        ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningState,
        ProvisionedRemoteComputeProvisioningStatus, ProvisionedRemoteComputeResources,
        ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
    },
};
use crate::workflow_catalog::WorkflowCatalogService;

use super::{
    coordination::ProvisionedRemoteComputeCoordinator,
    errors::ProvisionedRemoteComputeError,
    helpers::{
        failed_workspace_from_result, ignore_expected_error, remote_runtime, reset_remote_state,
        with_cleanup_failure, with_provisioning_failure, with_status_and_resources,
    },
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, ProvisionedRemoteComputeProvider, StartProvisionerParams,
        TerminateProvisionerParams,
    },
    registry::ProvisionedRemoteComputeProviderRegistry,
};

pub struct SetupProvisionedRemoteComputeWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset: WorkflowPreset,
    pub remote_placement: RemotePlacementPlan,
}

pub struct ProvisionedRemoteComputeService {
    provider_registry: ProvisionedRemoteComputeProviderRegistry,
    workflow_catalog_service: WorkflowCatalogService,
    coordinator: ProvisionedRemoteComputeCoordinator,
}

impl ProvisionedRemoteComputeService {
    pub fn new(
        provider_registry: ProvisionedRemoteComputeProviderRegistry,
        workflow_catalog_service: WorkflowCatalogService,
    ) -> Self {
        Self {
            provider_registry,
            workflow_catalog_service,
            coordinator: ProvisionedRemoteComputeCoordinator::default(),
        }
    }

    pub fn setup_workspace(
        &self,
        request: SetupProvisionedRemoteComputeWorkspaceRequest,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        if request.workspace_id.trim().is_empty() {
            return Err(
                ProvisionedRemoteComputeError::SetupWorkspaceInvalidRequest {
                    message: "workspace id is required".to_string(),
                },
            );
        }

        Ok(Workspace {
            id: request.workspace_id,
            workflow_preset: request.workflow_preset,
            runtime: WorkspaceRuntime::ProvisionedRemoteCompute(
                ProvisionedRemoteComputeWorkspace {
                    remote_placement: request.remote_placement,
                    provisioning: ProvisionedRemoteComputeProvisioningState {
                        status: ProvisionedRemoteComputeProvisioningStatus::NotStarted,
                        percent: None,
                    },
                    resources: ProvisionedRemoteComputeResources {
                        volume: None,
                        provisioner: None,
                        endpoint: None,
                    },
                },
            ),
        })
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<RemotePlacementOptions, ProvisionedRemoteComputeError> {
        let provider = self.provider_registry.for_provider(provider_id)?;

        provider.get_provider_placement_options().await
    }

    pub async fn provision_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let remote = remote_runtime(workspace)?;

        if matches!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Completed
                | ProvisionedRemoteComputeProvisioningStatus::Failed { .. }
        ) {
            return self.handle_terminal_status(workspace);
        }

        if matches!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling { .. }
        ) && remote.resources.endpoint.is_none()
            && remote.resources.provisioner.is_none()
            && remote.resources.volume.is_none()
        {
            return Ok(reset_remote_state(workspace));
        }

        let Some(_guard) = self.coordinator.try_enter(&workspace.id) else {
            return Err(ProvisionedRemoteComputeError::ProvisioningAlreadyRunning {
                workspace_id: workspace.id.clone(),
            });
        };

        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        match &remote.provisioning.status {
            ProvisionedRemoteComputeProvisioningStatus::NotStarted => {
                self.handle_not_started(workspace, remote, provider)
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: phase @ ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
            } => {
                self.handle_starting_provisioner(workspace, remote, provider, phase)
                    .await
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase:
                    phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    },
            } => {
                self.handle_cleaning_up_provisioner(workspace, remote, provider, phase)
                    .await
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase:
                    phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        ..
                    },
            } => {
                self.handle_running_provisioner(workspace, remote, provider, phase)
                    .await
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: phase @ ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
            } => {
                self.handle_creating_endpoint(workspace, remote, provider, phase)
                    .await
            }
            ProvisionedRemoteComputeProvisioningStatus::Completed
            | ProvisionedRemoteComputeProvisioningStatus::Failed { .. } => {
                self.handle_terminal_status(workspace)
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
            } => {
                self.handle_creating_volume(workspace, remote, provider)
                    .await
            }
            ProvisionedRemoteComputeProvisioningStatus::Cancelling { phase } => {
                self.handle_cancelling(workspace, remote, provider, phase.clone())
                    .await
            }
        }
    }

    fn handle_terminal_status(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        Ok(workspace.clone())
    }

    fn handle_not_started(
        &self,
        workspace: &Workspace,
        _remote: &ProvisionedRemoteComputeWorkspace,
        _provider: &dyn ProvisionedRemoteComputeProvider,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        Ok(with_status_and_resources(
            workspace,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
            },
            0,
            |_| {},
        ))
    }

    async fn handle_creating_volume(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let volume = match provider
            .create_volume(CreateVolumeParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                size_bytes: remote.remote_placement.volume_size_bytes,
                mount_path: "/workspace".to_string(),
            })
            .await
        {
            Ok(volume) => volume,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
                    error.into(),
                ));
            }
        };

        Ok(with_status_and_resources(
            workspace,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
            },
            25,
            |resources| {
                resources.volume = Some(volume);
            },
        ))
    }

    async fn handle_starting_provisioner(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
        phase: &ProvisionedRemoteComputeProvisioningPhase,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let volume = match remote.resources.volume.as_ref() {
            Some(volume) => volume,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                        message: "remote volume snapshot is required before provisioner start"
                            .to_string(),
                    },
                ));
            }
        };
        let provisioner_image_ref = match self.resolve_provisioner_image_ref(workspace, remote) {
            Ok(image_ref) => image_ref,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error,
                ));
            }
        };

        let provisioner = match provider
            .start_provisioner(StartProvisionerParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                volume_id: volume.id.clone(),
                provisioner_image_ref,
                mount_path: "/workspace".to_string(),
                requires_hugging_face_api_key: workspace
                    .workflow_preset
                    .requires_hugging_face_api_key,
            })
            .await
        {
            Ok(provisioner) => provisioner,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error.into(),
                ));
            }
        };

        Ok(with_status_and_resources(
            workspace,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::Pending,
                },
            },
            50,
            |resources| {
                resources.provisioner = Some(provisioner);
            },
        ))
    }

    async fn handle_cleaning_up_provisioner(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
        phase: &ProvisionedRemoteComputeProvisioningPhase,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let provisioner = match remote.resources.provisioner.as_ref() {
            Some(provisioner) => provisioner,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                        message:
                            "remote provisioner snapshot is required before provisioner cleanup"
                                .to_string(),
                    },
                ));
            }
        };
        let status = match provider
            .get_provisioner_status(GetProvisionerStatusParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: provisioner.id.clone(),
                status_url: provisioner.status_url.clone(),
            })
            .await
        {
            Ok(status) => status,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error.into(),
                ));
            }
        };

        if !matches!(
            status,
            ProvisionedRemoteComputeProvisionerStatus::Succeeded
                | ProvisionedRemoteComputeProvisionerStatus::Failed { .. }
        ) {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!("cleanup requires finished provisioner status: {status:?}"),
                },
            ));
        }

        let termination_result = provider
            .terminate_provisioner(TerminateProvisionerParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: provisioner.id.clone(),
            })
            .await;

        match (status, termination_result) {
            (ProvisionedRemoteComputeProvisionerStatus::Succeeded, Ok(())) => {
                Ok(with_status_and_resources(
                    workspace,
                    ProvisionedRemoteComputeProvisioningStatus::InProgress {
                        phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
                    },
                    75,
                    |resources| {
                        resources.provisioner = None;
                    },
                ))
            }
            (ProvisionedRemoteComputeProvisionerStatus::Succeeded, Err(error)) => {
                Ok(with_provisioning_failure(
                    workspace,
                    Some(
                        ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                            status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                        },
                    ),
                    error.into(),
                ))
            }
            (ProvisionedRemoteComputeProvisionerStatus::Failed { code, message }, Ok(())) => {
                let mut workspace = with_provisioning_failure(
                    workspace,
                    Some(
                        ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                            status: ProvisionedRemoteComputeProvisionerStatus::Failed {
                                code,
                                message,
                            },
                        },
                    ),
                    ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
                );
                let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
                remote.resources.provisioner = None;
                Ok(workspace)
            }
            (ProvisionedRemoteComputeProvisionerStatus::Failed { code, message }, Err(_)) => {
                Ok(with_provisioning_failure(
                    workspace,
                    Some(
                        ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                            status: ProvisionedRemoteComputeProvisionerStatus::Failed {
                                code,
                                message,
                            },
                        },
                    ),
                    ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
                ))
            }
            _ => unreachable!("cleanup-ready status was validated before termination"),
        }
    }

    async fn handle_running_provisioner(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
        phase: &ProvisionedRemoteComputeProvisioningPhase,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let provisioner = match remote.resources.provisioner.as_ref() {
            Some(provisioner) => provisioner,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                        message: "remote provisioner snapshot is required before status polling"
                            .to_string(),
                    },
                ));
            }
        };
        let status = match provider
            .get_provisioner_status(GetProvisionerStatusParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: provisioner.id.clone(),
                status_url: provisioner.status_url.clone(),
            })
            .await
        {
            Ok(status) => status,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error.into(),
                ));
            }
        };

        let percent = match &status {
            ProvisionedRemoteComputeProvisionerStatus::Pending
            | ProvisionedRemoteComputeProvisionerStatus::Starting => 50,
            ProvisionedRemoteComputeProvisionerStatus::Running => 60,
            ProvisionedRemoteComputeProvisionerStatus::Succeeded
            | ProvisionedRemoteComputeProvisionerStatus::Failed { .. } => 75,
            ProvisionedRemoteComputeProvisionerStatus::CleaningUp => 75,
        };
        let provisioning_status = match status {
            ProvisionedRemoteComputeProvisionerStatus::Succeeded
            | ProvisionedRemoteComputeProvisionerStatus::Failed { .. } => {
                ProvisionedRemoteComputeProvisioningStatus::InProgress {
                    phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    },
                }
            }
            status => ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status,
                },
            },
        };

        Ok(with_status_and_resources(
            workspace,
            provisioning_status,
            percent,
            |_| {},
        ))
    }

    async fn handle_creating_endpoint(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
        phase: &ProvisionedRemoteComputeProvisioningPhase,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let volume = match remote.resources.volume.as_ref() {
            Some(volume) => volume,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                        message: "remote volume snapshot is required before endpoint creation"
                            .to_string(),
                    },
                ));
            }
        };
        let endpoint_image_ref = match self.resolve_endpoint_image_ref(workspace, remote) {
            Ok(image_ref) => image_ref,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error,
                ));
            }
        };
        let endpoint = match provider
            .create_endpoint(CreateEndpointParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                volume_id: volume.id.clone(),
                endpoint_image_ref,
                mount_path: "/workspace".to_string(),
                keep_alive_limits: remote.remote_placement.keep_alive_limits.clone(),
            })
            .await
        {
            Ok(endpoint) => endpoint,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    error.into(),
                ));
            }
        };

        Ok(with_status_and_resources(
            workspace,
            ProvisionedRemoteComputeProvisioningStatus::Completed,
            100,
            |resources| {
                resources.endpoint = Some(endpoint);
            },
        ))
    }

    async fn handle_cancelling(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        provider: &dyn ProvisionedRemoteComputeProvider,
        phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        if let Some(endpoint) = remote.resources.endpoint.as_ref() {
            return match ignore_expected_error(
                provider
                    .delete_endpoint(DeleteEndpointParams {
                        workspace_id: workspace.id.clone(),
                        endpoint_id: endpoint.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteEndpointNotFound,
            ) {
                Ok(()) => Ok(with_status_and_resources(
                    workspace,
                    ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                        phase: Some(
                            ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                            },
                        ),
                    },
                    75,
                    |resources| {
                        resources.endpoint = None;
                    },
                )),
                Err(error) => Ok(with_cleanup_failure(
                    workspace,
                    Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                    error,
                )),
            };
        }

        if let Some(provisioner) = remote.resources.provisioner.as_ref() {
            let attempted_phase = match phase {
                Some(ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status,
                }) => Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status },
                ),
                _ => Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    },
                ),
            };
            return match ignore_expected_error(
                provider
                    .terminate_provisioner(TerminateProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        provisioner_id: provisioner.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteProvisionerNotFound,
            ) {
                Ok(()) => Ok(with_status_and_resources(
                    workspace,
                    ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                        phase: Some(
                            ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
                        ),
                    },
                    25,
                    |resources| {
                        resources.provisioner = None;
                    },
                )),
                Err(error) => Ok(with_cleanup_failure(workspace, attempted_phase, error)),
            };
        }

        if let Some(volume) = remote.resources.volume.as_ref() {
            return match ignore_expected_error(
                provider
                    .delete_volume(DeleteVolumeParams {
                        workspace_id: workspace.id.clone(),
                        volume_id: volume.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteVolumeNotFound,
            ) {
                Ok(()) => Ok(reset_remote_state(workspace)),
                Err(error) => Ok(with_cleanup_failure(
                    workspace,
                    Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
                    error,
                )),
            };
        }

        Ok(reset_remote_state(workspace))
    }

    pub fn cancel_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let remote = remote_runtime(workspace)?;

        let ProvisionedRemoteComputeProvisioningStatus::InProgress { phase } =
            &remote.provisioning.status
        else {
            return Ok(with_provisioning_failure(
                workspace,
                None,
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            ));
        };

        let mut workspace = workspace.clone();
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(phase.clone()),
        };
        Ok(workspace)
    }

    fn resolve_provisioner_image_ref(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract =
            self.resolve_runtime_contract_reference(workspace, remote, |requirements| {
                &requirements.provisioner_contract
            })?;
        let catalog = self
            .workflow_catalog_service
            .get_provisioner_contract_catalog()
            .map_err(|error| {
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!("provisioner contract catalog is invalid: {error:?}"),
                }
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "provisioner contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    fn resolve_endpoint_image_ref(
        &self,
        workspace: &Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract =
            self.resolve_runtime_contract_reference(workspace, remote, |requirements| {
                &requirements.endpoint_contract
            })?;
        let catalog = self
            .workflow_catalog_service
            .get_endpoint_contract_catalog()
            .map_err(|error| {
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!("endpoint contract catalog is invalid: {error:?}"),
                }
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "endpoint contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    fn resolve_runtime_contract_reference<'a>(
        &self,
        workspace: &'a Workspace,
        remote: &ProvisionedRemoteComputeWorkspace,
        contract: impl FnOnce(
            &'a crate::domain::workflow_preset::RemoteProviderRuntimeRequirements,
        ) -> &'a RuntimeContractReference,
    ) -> Result<&'a RuntimeContractReference, ProvisionedRemoteComputeProvisioningError> {
        let provider_requirements = workspace
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(remote.remote_placement.gpu_cloud_provider_id)
            .ok_or_else(
                || ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: format!(
                        "workflow preset has no runtime requirements for provider {:?}",
                        remote.remote_placement.gpu_cloud_provider_id
                    ),
                },
            )?;

        Ok(contract(provider_requirements))
    }

    pub fn execute_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<(), ProvisionedRemoteComputeError> {
        let remote = remote_runtime(workspace)?;

        if remote.provisioning.status != ProvisionedRemoteComputeProvisioningStatus::Completed {
            return Err(ProvisionedRemoteComputeError::ExecuteWorkspaceNotReady);
        }

        if remote.resources.endpoint.is_none() {
            return Err(ProvisionedRemoteComputeError::ExecuteWorkspaceMissingEndpoint);
        }

        Err(
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotImplemented {
                message: "endpoint worker execution is not implemented in this skeleton"
                    .to_string(),
            },
        )
    }

    pub async fn cleanup_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        let remote = remote_runtime(workspace)?;
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        let endpoint_cleanup = match &remote.resources.endpoint {
            Some(endpoint) => failed_workspace_from_result(
                workspace,
                provider
                    .delete_endpoint(DeleteEndpointParams {
                        workspace_id: workspace.id.clone(),
                        endpoint_id: endpoint.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteEndpointNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = endpoint_cleanup {
            return Ok(failed_workspace);
        }

        let provisioner_cleanup = match &remote.resources.provisioner {
            Some(provisioner) => failed_workspace_from_result(
                workspace,
                provider
                    .terminate_provisioner(TerminateProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        provisioner_id: provisioner.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteProvisionerNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = provisioner_cleanup {
            return Ok(failed_workspace);
        }

        let volume_cleanup = match &remote.resources.volume {
            Some(volume) => failed_workspace_from_result(
                workspace,
                provider
                    .delete_volume(DeleteVolumeParams {
                        workspace_id: workspace.id.clone(),
                        volume_id: volume.id.clone(),
                    })
                    .await,
                ProvisionedRemoteComputeError::RemoteVolumeNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = volume_cleanup {
            return Ok(failed_workspace);
        }

        Ok(reset_remote_state(workspace))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        placement::{
            RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits,
            RemoteGpuPlacementOption, RemotePlacementOptions, RemotePlacementPlan,
        },
        provider::{GpuCloudProviderId, ProviderApiError},
        runtime_contract::RuntimeContractReference,
        workflow_preset::{
            RemoteProviderRuntimeRequirements, RemoteRuntimeRequirements, WorkflowExecutionType,
            WorkflowPreset,
        },
        workspace::{
            ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeProvisionerSnapshot,
            ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
            ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeResources,
            ProvisionedRemoteComputeVolumeSnapshot, WorkspaceRuntime,
        },
    };

    use super::*;
    use crate::provisioned_remote_compute::{
        errors::ProvisionedRemoteComputeError,
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, ProvisionedRemoteComputeEndpointProvider,
            ProvisionedRemoteComputePlacementOptionsProvider, ProvisionedRemoteComputeProvider,
            ProvisionedRemoteComputeProvisionerProvider, ProvisionedRemoteComputeVolumeProvider,
            StartProvisionerParams, TerminateProvisionerParams,
        },
    };
    use crate::shared::AppFuture;

    #[derive(Default)]
    struct ProviderState {
        calls: Vec<&'static str>,
        placement_options_result:
            Option<Result<RemotePlacementOptions, ProvisionedRemoteComputeError>>,
        create_volume_error: Option<ProvisionedRemoteComputeError>,
        create_endpoint_error: Option<ProvisionedRemoteComputeError>,
        start_provisioner_error: Option<ProvisionedRemoteComputeError>,
        delete_endpoint_error: Option<ProvisionedRemoteComputeError>,
        terminate_provisioner_error: Option<ProvisionedRemoteComputeError>,
        delete_volume_error: Option<ProvisionedRemoteComputeError>,
        provisioner_status_results:
            Vec<Result<ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeError>>,
        last_create_volume_params: Option<CreateVolumeParams>,
        last_create_endpoint_params: Option<CreateEndpointParams>,
        last_start_provisioner_params: Option<StartProvisionerParams>,
        last_get_provisioner_status_params: Option<GetProvisionerStatusParams>,
    }

    fn provider_request_failed(message: &str) -> ProvisionedRemoteComputeError {
        ProviderApiError::RequestFailed {
            message: message.to_string(),
        }
        .into()
    }

    fn placement_options() -> RemotePlacementOptions {
        RemotePlacementOptions {
            max_persistent_storage_volume_size_bytes: Some(10),
            datacenters: vec![RemoteDatacenterPlacementOption {
                id: "dc".to_string(),
                name: "Datacenter".to_string(),
                gpu_options: vec![RemoteGpuPlacementOption {
                    id: "gpu".to_string(),
                    name: "GPU".to_string(),
                    vram_bytes: 24,
                    availability_score: 90,
                }],
            }],
        }
    }

    struct FakeProvider {
        state: Arc<Mutex<ProviderState>>,
    }

    impl FakeProvider {
        fn new(state: Arc<Mutex<ProviderState>>) -> Self {
            Self { state }
        }
    }

    impl ProvisionedRemoteComputePlacementOptionsProvider for FakeProvider {
        fn get_provider_placement_options<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<RemotePlacementOptions, ProvisionedRemoteComputeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("get_provider_placement_options");

                state
                    .placement_options_result
                    .clone()
                    .unwrap_or_else(|| Ok(placement_options()))
            })
        }
    }

    impl ProvisionedRemoteComputeVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            params: CreateVolumeParams,
        ) -> AppFuture<
            'a,
            Result<ProvisionedRemoteComputeVolumeSnapshot, ProvisionedRemoteComputeError>,
        > {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("create_volume");
                state.last_create_volume_params = Some(params);

                if let Some(error) = state.create_volume_error.clone() {
                    return Err(error);
                }

                Ok(ProvisionedRemoteComputeVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> AppFuture<'a, Result<(), ProvisionedRemoteComputeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("delete_volume");
                if let Some(error) = state.delete_volume_error.take() {
                    return Err(error);
                }
                Ok(())
            })
        }
    }

    impl ProvisionedRemoteComputeProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            params: StartProvisionerParams,
        ) -> AppFuture<
            'a,
            Result<ProvisionedRemoteComputeProvisionerSnapshot, ProvisionedRemoteComputeError>,
        > {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("start_provisioner");
                state.last_start_provisioner_params = Some(params);

                if let Some(error) = state.start_provisioner_error.clone() {
                    return Err(error);
                }

                Ok(ProvisionedRemoteComputeProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> AppFuture<'a, Result<(), ProvisionedRemoteComputeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("terminate_provisioner");
                if let Some(error) = state.terminate_provisioner_error.take() {
                    return Err(error);
                }
                Ok(())
            })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            params: GetProvisionerStatusParams,
        ) -> AppFuture<
            'a,
            Result<ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeError>,
        > {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("get_provisioner_status");
                state.last_get_provisioner_status_params = Some(params);
                if state.provisioner_status_results.is_empty() {
                    return Ok(ProvisionedRemoteComputeProvisionerStatus::Pending);
                }
                state.provisioner_status_results.remove(0)
            })
        }
    }

    impl ProvisionedRemoteComputeEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            params: CreateEndpointParams,
        ) -> AppFuture<
            'a,
            Result<ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeError>,
        > {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("create_endpoint");
                state.last_create_endpoint_params = Some(params);
                if let Some(error) = state.create_endpoint_error.clone() {
                    return Err(error);
                }
                Ok(ProvisionedRemoteComputeEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _params: DeleteEndpointParams,
        ) -> AppFuture<'a, Result<(), ProvisionedRemoteComputeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("delete_endpoint");
                if let Some(error) = state.delete_endpoint_error.take() {
                    return Err(error);
                }
                Ok(())
            })
        }
    }

    impl ProvisionedRemoteComputeProvider for FakeProvider {
        fn provider_id(&self) -> GpuCloudProviderId {
            GpuCloudProviderId::Runpod
        }
    }

    fn service_with_state(state: Arc<Mutex<ProviderState>>) -> ProvisionedRemoteComputeService {
        ProvisionedRemoteComputeService::new(
            ProvisionedRemoteComputeProviderRegistry::new(vec![Box::new(FakeProvider::new(state))]),
            WorkflowCatalogService::new(),
        )
    }

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "preset".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: false,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 1,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "comfyui-hidream-o1-dev".to_string(),
                        version: "1.0.15".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "luma-forge-provisioner".to_string(),
                        version: "1.0.6".to_string(),
                    },
                }],
            },
            required_model_assets: vec![],
        }
    }

    fn placement_plan() -> RemotePlacementPlan {
        RemotePlacementPlan {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            volume_size_bytes: 1,
            keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
                default_seconds: 60,
                min_seconds: 30,
                max_seconds: 120,
            }),
        }
    }

    fn draft_workspace(service: &ProvisionedRemoteComputeService) -> Workspace {
        service
            .setup_workspace(SetupProvisionedRemoteComputeWorkspaceRequest {
                workspace_id: "workspace".to_string(),
                workflow_preset: workflow_preset(),
                remote_placement: placement_plan(),
            })
            .expect("workspace setup should succeed")
    }

    fn workspace_with_all_remote_resources(service: &ProvisionedRemoteComputeService) -> Workspace {
        let mut workspace = draft_workspace(service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.resources.endpoint = Some(ProvisionedRemoteComputeEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        });
        workspace
    }

    #[test]
    fn setup_workspace_returns_provisioned_remote_compute_with_not_started_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));

        let workspace = draft_workspace(&service);

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = workspace.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn get_provider_placement_options_returns_selected_provider_options() {
        let state = Arc::new(Mutex::new(ProviderState {
            placement_options_result: Some(Ok(placement_options())),
            ..ProviderState::default()
        }));
        let service = service_with_state(state.clone());

        let options = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
            .expect("placement options should be returned");

        assert_eq!(options, placement_options());
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provider_placement_options"]
        );
    }

    #[test]
    fn get_provider_placement_options_returns_provider_unavailable() {
        let service = ProvisionedRemoteComputeService::new(
            ProvisionedRemoteComputeProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );

        let error = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
            .expect_err("missing provider should fail");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ProviderUnavailable {
                provider_id: GpuCloudProviderId::Runpod
            }
        );
    }

    #[test]
    fn remote_provisioning_error_has_cancellation_cleanup_failed_variant() {
        assert_eq!(
            ProvisionedRemoteComputeProvisioningError::CancellationCleanupFailed,
            ProvisionedRemoteComputeProvisioningError::CancellationCleanupFailed
        );
    }

    #[test]
    fn cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
        };
        remote.provisioning.percent = Some(25);

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("in-progress workspace should enter cancellation");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner)
            }
        );
        assert_eq!(remote.provisioning.percent, Some(25));
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn cancel_workspace_not_started_marks_invalid_state_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("invalid cancellation should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: None,
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn cancel_workspace_completed_marks_invalid_state_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Completed;
        remote.provisioning.percent = Some(100);

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("invalid cancellation should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: None,
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn cancel_workspace_failed_marks_invalid_state_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Failed {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
            error: ProvisionedRemoteComputeProvisioningError::Provider(
                ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                },
            ),
        };

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("invalid cancellation should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: None,
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_not_started_marks_creating_volume_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("not started workspace should enter volume creation");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(remote.resources.volume, None);
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(remote.resources.endpoint, None);
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume
            }
        );
        assert_eq!(remote.provisioning.percent, Some(0));
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_creating_volume_creates_volume_only() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
        };
        remote.provisioning.percent = Some(0);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("creating volume workspace should create a volume");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string()
            })
        );
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(remote.resources.endpoint, None);
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner
            }
        );
        assert_eq!(remote.provisioning.percent, Some(25));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["create_volume"]
        );
        assert_eq!(
            state
                .lock()
                .expect("state lock should succeed")
                .last_create_volume_params,
            Some(CreateVolumeParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                size_bytes: 1,
                mount_path: "/workspace".to_string(),
            })
        );
    }

    #[test]
    fn provision_workspace_rejects_duplicate_in_flight_workspace_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
        };
        remote.provisioning.percent = Some(0);

        let guard = service
            .coordinator
            .try_enter(&workspace.id)
            .expect("first workspace entry should be accepted");
        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("duplicate workspace provisioning should be rejected");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ProvisioningAlreadyRunning {
                workspace_id: "workspace".to_string(),
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());

        drop(guard);
        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("workspace provisioning should proceed after guard release");
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string()
            })
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["create_volume"]
        );
    }

    #[test]
    fn provision_workspace_missing_provider_returns_error_without_failed_workspace() {
        let service = ProvisionedRemoteComputeService::new(
            ProvisionedRemoteComputeProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );
        let workspace = draft_workspace(&service);

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("missing provider should return service error");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ProviderUnavailable {
                provider_id: GpuCloudProviderId::Runpod,
            }
        );
    }

    #[test]
    fn provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should delete endpoint");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(remote.resources.endpoint, None);
        assert_eq!(
            remote.resources.provisioner,
            Some(ProvisionedRemoteComputeProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                )
            }
        );
        assert_eq!(remote.provisioning.percent, Some(75));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_missing_endpoint_skips_to_provisioner_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("missing endpoint should skip to provisioner cleanup");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(remote.resources.endpoint, None);
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner)
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_ignores_endpoint_not_found() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(ProvisionedRemoteComputeError::RemoteEndpointNotFound),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("endpoint not found should be treated as already deleted");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(remote.resources.endpoint, None);
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                )
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_terminates_provisioner_without_polling_status() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(
                ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::Running,
                },
            ),
        };
        remote.provisioning.percent = Some(60);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should terminate provisioner");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner)
            }
        );
        assert_eq!(remote.provisioning.percent, Some(25));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_deletes_volume_and_resets_to_not_started() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
        };
        remote.provisioning.percent = Some(25);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should delete volume");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_volume"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_endpoint_cleanup_failure_marks_failed_and_preserves_snapshots(
    ) {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: Some(ProvisionedRemoteComputeVolumeSnapshot {
                    id: "volume".to_string(),
                }),
                provisioner: Some(ProvisionedRemoteComputeProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                }),
                endpoint: Some(ProvisionedRemoteComputeEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                }),
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_provisioner_cleanup_failure_marks_failed_and_preserves_snapshots(
    ) {
        let state = Arc::new(Mutex::new(ProviderState {
            terminate_provisioner_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(
                ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::Running,
                },
            ),
        };
        remote.provisioning.percent = Some(60);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources.provisioner,
            Some(ProvisionedRemoteComputeProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Running,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_volume_cleanup_failure_marks_failed_and_preserves_snapshot() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_volume_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
        };
        remote.provisioning.percent = Some(25);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_volume"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_endpoint_failure_reports_attempted_phase() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_provisioner_failure_reports_attempted_phase() {
        let state = Arc::new(Mutex::new(ProviderState {
            terminate_provisioner_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_unexpected_cleanup_error_maps_to_cancellation_cleanup_failed()
    {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(ProvisionedRemoteComputeError::RemoteVolumeNotFound),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error: ProvisionedRemoteComputeProvisioningError::CancellationCleanupFailed,
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_cancelling_without_resources_resets_to_not_started_without_provider_calls(
    ) {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
        };
        remote.provisioning.percent = Some(10);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("empty cancellation should reset workspace");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_cancelling_without_resources_resets_without_provider_lookup() {
        let service = ProvisionedRemoteComputeService::new(
            ProvisionedRemoteComputeProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
        };
        remote.provisioning.percent = Some(10);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("empty cancellation should reset without provider lookup");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cancelled.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
    }

    #[test]
    fn provision_workspace_starting_provisioner_advances_one_step() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
        };
        remote.provisioning.percent = Some(25);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("starting provisioner phase should start provisioner");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.provisioner,
            Some(ProvisionedRemoteComputeProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::Pending
                }
            }
        );
        assert_eq!(remote.provisioning.percent, Some(50));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["start_provisioner"]
        );
        assert_eq!(
            state
                .lock()
                .expect("state lock should succeed")
                .last_start_provisioner_params,
            Some(StartProvisionerParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_id: "volume".to_string(),
                provisioner_image_ref: "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:8e0d74276a36db8b0fae428b492e8fd080eea5311a7d153a0d60023c7e5a8295"
                    .to_string(),
                mount_path: "/workspace".to_string(),
                requires_hugging_face_api_key: false,
            })
        );
    }

    #[test]
    fn cleaning_up_status_is_plain_provisioner_status() {
        let status = ProvisionedRemoteComputeProvisionerStatus::CleaningUp;

        assert_eq!(
            status,
            ProvisionedRemoteComputeProvisionerStatus::CleaningUp
        );
    }

    #[test]
    fn provision_workspace_running_provisioner_stores_incomplete_status() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Running,
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Pending,
            },
        };
        remote.provisioning.percent = Some(50);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("running provisioner should poll status");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::Running
                }
            }
        );
        assert_eq!(remote.provisioning.percent, Some(60));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status"]
        );
    }

    #[test]
    fn provision_workspace_worker_success_moves_to_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Succeeded,
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("worker success should move to cleanup");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp
                }
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status"]
        );
    }

    #[test]
    fn provision_workspace_worker_failure_moves_to_cleanup_with_failure_details() {
        let failed_status = ProvisionedRemoteComputeProvisionerStatus::Failed {
            code: "provisioner_worker_asset_download_failed".to_string(),
            message: "asset download failed".to_string(),
        };
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(failed_status.clone())],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("worker failure should move to cleanup before failed state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp
                }
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status"]
        );
    }

    #[test]
    fn provision_workspace_cleanup_after_success_moves_to_endpoint_creation() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Succeeded,
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup after success should terminate provisioner");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint
            }
        );
        assert_eq!(remote.provisioning.percent, Some(75));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status", "terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cleanup_after_worker_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Failed {
                    code: "provisioner_worker_step_timeout".to_string(),
                    message: "step timed out".to_string(),
                },
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup after worker failure should mark failed");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(remote.resources.provisioner, None);
        assert_eq!(
            remote.resources.volume,
            Some(ProvisionedRemoteComputeVolumeSnapshot {
                id: "volume".to_string()
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Failed {
                            code: "provisioner_worker_step_timeout".to_string(),
                            message: "step timed out".to_string(),
                        },
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_error_after_success_marks_failed_and_preserves_provisioner() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Succeeded,
            )],
            terminate_provisioner_error: Some(provider_request_failed("terminate failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup error after success should become failed workspace");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.provisioner,
            Some(ProvisionedRemoteComputeProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "terminate failed".to_string(),
                    }
                ),
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_error_after_worker_failure_preserves_worker_failure() {
        let failed_status = ProvisionedRemoteComputeProvisionerStatus::Failed {
            code: "provisioner_worker_unexpected_error".to_string(),
            message: "unexpected worker error".to_string(),
        };
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(failed_status.clone())],
            terminate_provisioner_error: Some(provider_request_failed("terminate failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup error after worker failure should preserve worker failure");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.provisioner,
            Some(ProvisionedRemoteComputeProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: failed_status,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_with_incomplete_status_returns_invalid_state_without_termination(
    ) {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(
                ProvisionedRemoteComputeProvisionerStatus::Running,
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("incomplete cleanup status should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "cleanup requires finished provisioner status: Running".to_string(),
                },
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status"]
        );
    }

    #[test]
    fn provision_workspace_returns_provider_request_failed_messages() {
        let state = Arc::new(Mutex::new(ProviderState {
            create_volume_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
        };
        remote.provisioning.percent = Some(0);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("provider request failure should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "provider request failed".to_string(),
                    }
                ),
            }
        );
    }

    #[test]
    fn provision_workspace_start_provisioner_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            start_provisioner_error: Some(provider_request_failed("start failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("start provisioner failure should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "start failed".to_string(),
                    }
                ),
            }
        );
    }

    #[test]
    fn provision_workspace_status_poll_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Err(provider_request_failed("status failed"))],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("status poll failure should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Running,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "status failed".to_string(),
                    }
                ),
            }
        );
    }

    #[test]
    fn running_provisioner_worker_error_is_recorded_as_worker_error() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Err(
                ProvisionedRemoteComputeError::ProvisionerWorker(
                    ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnauthorized,
                ),
            )],
            ..ProviderState::default()
        }));
        let service = service_with_state(state);
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.provisioner = Some(ProvisionedRemoteComputeProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://provisioner.example/status".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Running,
            },
        };

        let result = block_on(service.provision_workspace(&workspace))
            .expect("worker error should be converted into failed workspace");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = result.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Running,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnauthorized,
            }
        );
    }

    #[test]
    fn provision_workspace_create_endpoint_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            create_endpoint_error: Some(provider_request_failed("endpoint failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("create endpoint failure should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error: ProvisionedRemoteComputeProvisioningError::Provider(
                    ProviderApiError::RequestFailed {
                        message: "endpoint failed".to_string(),
                    }
                ),
            }
        );
    }

    #[test]
    fn provision_workspace_completed_returns_workspace_unchanged_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Completed;
        remote.provisioning.percent = Some(100);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("completed workspace should be returned unchanged");

        assert_eq!(provisioned, workspace);
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_failed_returns_workspace_unchanged_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Failed {
            phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
            error: ProvisionedRemoteComputeProvisioningError::Provider(
                ProviderApiError::RequestFailed {
                    message: "raw failure".to_string(),
                },
            ),
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("failed workspace should be returned unchanged");

        assert_eq!(provisioned, workspace);
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_creating_endpoint_marks_completed() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.resources.volume = Some(ProvisionedRemoteComputeVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("endpoint creation should complete workspace");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.resources.endpoint,
            Some(ProvisionedRemoteComputeEndpointSnapshot {
                id: "endpoint".to_string(),
                url: "https://endpoint.example".to_string(),
            })
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Completed
        );
        assert_eq!(remote.provisioning.percent, Some(100));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["create_endpoint"]
        );
        assert_eq!(
            state
                .lock()
                .expect("state lock should succeed")
                .last_create_endpoint_params,
            Some(CreateEndpointParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_id: "volume".to_string(),
                endpoint_image_ref: "ghcr.io/p-shapov/luma-forge/runpod-endpoint-worker@sha256:ac7b4ee14423f5e74f444a03c429dece830fc4f72b01847df18b2a5b960cdd1a"
                    .to_string(),
                mount_path: "/workspace".to_string(),
                keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
                    default_seconds: 60,
                    min_seconds: 30,
                    max_seconds: 120,
                }),
            })
        );
    }

    #[test]
    fn provision_workspace_creating_endpoint_without_volume_returns_invalid_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing volume should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote volume snapshot is required before endpoint creation"
                        .to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_starting_provisioner_without_volume_returns_invalid_state_without_provider_calls(
    ) {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing volume should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote volume snapshot is required before provisioner start"
                        .to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_running_provisioner_without_snapshot_returns_invalid_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing provisioner should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Running,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote provisioner snapshot is required before status polling"
                        .to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_cleanup_without_provisioner_returns_invalid_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing provisioner should fail provisioning state");

        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = provisioned.runtime;
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::Failed {
                phase: Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    }
                ),
                error: ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote provisioner snapshot is required before provisioner cleanup"
                        .to_string(),
                },
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn execute_workspace_rejects_non_ready_workspace() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let error = service
            .execute_workspace(&workspace)
            .expect_err("draft workspace should not be executed");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotReady
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn execute_workspace_completed_without_endpoint_returns_missing_endpoint() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Completed;

        let error = service
            .execute_workspace(&workspace)
            .expect_err("completed workspace without endpoint should not execute");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ExecuteWorkspaceMissingEndpoint
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn execute_workspace_completed_with_endpoint_returns_not_implemented() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Completed;
        remote.resources.endpoint = Some(ProvisionedRemoteComputeEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        });

        let error = service
            .execute_workspace(&workspace)
            .expect_err("endpoint execution is not implemented yet");

        assert_eq!(
            error,
            ProvisionedRemoteComputeError::ExecuteWorkspaceNotImplemented {
                message: "endpoint worker execution is not implemented in this skeleton"
                    .to_string(),
            }
        );
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn cleanup_workspace_cleans_resources_in_dependency_order() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let cleaned_workspace = block_on(service.cleanup_workspace(&workspace))
            .expect("workspace cleanup should succeed");

        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
        );
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cleaned_workspace.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
    }

    #[test]
    fn cleanup_workspace_ignores_not_found_cleanup_errors() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(ProvisionedRemoteComputeError::RemoteEndpointNotFound),
            terminate_provisioner_error: Some(
                ProvisionedRemoteComputeError::RemoteProvisionerNotFound,
            ),
            delete_volume_error: Some(ProvisionedRemoteComputeError::RemoteVolumeNotFound),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let cleaned_workspace = block_on(service.cleanup_workspace(&workspace))
            .expect("not-found cleanup errors should be treated as already deleted");

        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
        );
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = cleaned_workspace.runtime;
        assert_eq!(
            remote.resources,
            ProvisionedRemoteComputeResources {
                volume: None,
                provisioner: None,
                endpoint: None,
            }
        );
        assert_eq!(
            remote.provisioning.status,
            ProvisionedRemoteComputeProvisioningStatus::NotStarted
        );
        assert_eq!(remote.provisioning.percent, None);
    }

    #[test]
    fn cleanup_workspace_endpoint_cleanup_failure_marks_failed_and_stops_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let failed_workspace = block_on(service.cleanup_workspace(&workspace))
            .expect("cleanup failure should return failed workspace");

        assert_eq!(
            failed_workspace,
            failed_cleanup_workspace(&workspace, "provider request failed")
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
    }

    #[test]
    fn cleanup_workspace_provisioner_cleanup_failure_marks_failed_and_stops_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState {
            terminate_provisioner_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let failed_workspace = block_on(service.cleanup_workspace(&workspace))
            .expect("cleanup failure should return failed workspace");

        assert_eq!(
            failed_workspace,
            failed_cleanup_workspace(&workspace, "provider request failed")
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner"]
        );
    }

    #[test]
    fn cleanup_workspace_volume_cleanup_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_volume_error: Some(provider_request_failed("provider request failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let failed_workspace = block_on(service.cleanup_workspace(&workspace))
            .expect("cleanup failure should return failed workspace");

        assert_eq!(
            failed_workspace,
            failed_cleanup_workspace(&workspace, "provider request failed")
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
        );
    }

    fn failed_cleanup_workspace(workspace: &Workspace, message: &str) -> Workspace {
        let mut workspace = workspace.clone();
        let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
        remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Failed {
            phase: None,
            error: ProvisionedRemoteComputeProvisioningError::Provider(
                ProviderApiError::RequestFailed {
                    message: message.to_string(),
                },
            ),
        };
        workspace
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::{
            future::Future,
            pin::Pin,
            task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
        };

        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}

            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);

        loop {
            match Pin::new(&mut future).poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => {}
            }
        }
    }
}
