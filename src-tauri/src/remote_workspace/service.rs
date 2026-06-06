use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::domain::{
    placement::{RemotePlacementOptions, RemotePlacementPlan},
    provider::GpuCloudProviderId,
    runtime_contract::RuntimeContractReference,
    workflow_preset::WorkflowPreset,
    workspace::{
        RemoteProvisionerStatus, RemoteProvisioningError, RemoteProvisioningPhase,
        RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
        RemoteWorkspaceResources, Workspace, WorkspaceRuntime,
    },
};
use crate::workflow_catalog::WorkflowCatalogService;

use super::{
    errors::RemoteWorkspaceError,
    helpers::{
        failed_workspace_from_result, ignore_expected_error, remote_runtime, reset_remote_state,
        with_cleanup_failure, with_provisioning_failure, with_status_and_resources,
    },
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, RemoteWorkspaceProvider, StartProvisionerParams,
        TerminateProvisionerParams,
    },
    registry::RemoteWorkspaceProviderRegistry,
};

pub struct SetupWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset: WorkflowPreset,
    pub remote_placement: RemotePlacementPlan,
}

pub struct RemoteWorkspaceService {
    provider_registry: RemoteWorkspaceProviderRegistry,
    workflow_catalog_service: WorkflowCatalogService,
    coordinator: RemoteWorkspaceProvisioningCoordinator,
}

impl RemoteWorkspaceService {
    pub fn new(
        provider_registry: RemoteWorkspaceProviderRegistry,
        workflow_catalog_service: WorkflowCatalogService,
    ) -> Self {
        Self {
            provider_registry,
            workflow_catalog_service,
            coordinator: RemoteWorkspaceProvisioningCoordinator::default(),
        }
    }

    pub fn setup_workspace(
        &self,
        request: SetupWorkspaceRequest,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        if request.workspace_id.trim().is_empty() {
            return Err(RemoteWorkspaceError::SetupWorkspaceInvalidRequest {
                message: "workspace id is required".to_string(),
            });
        }

        Ok(Workspace {
            id: request.workspace_id,
            workflow_preset: request.workflow_preset,
            runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
                remote_placement: request.remote_placement,
                remote_provisioning: RemoteProvisioningState {
                    status: RemoteProvisioningStatus::NotStarted,
                    percent: None,
                },
                remote_resources: RemoteWorkspaceResources {
                    remote_volume: None,
                    remote_provisioner: None,
                    remote_endpoint: None,
                },
            }),
        })
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<RemotePlacementOptions, RemoteWorkspaceError> {
        let provider = self.provider_registry.for_provider(provider_id)?;

        provider.get_provider_placement_options().await
    }

    pub async fn provision_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_runtime(workspace)?;

        if matches!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Completed | RemoteProvisioningStatus::Failed { .. }
        ) {
            return self.handle_terminal_status(workspace);
        }

        if matches!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling { .. }
        ) && remote.remote_resources.remote_endpoint.is_none()
            && remote.remote_resources.remote_provisioner.is_none()
            && remote.remote_resources.remote_volume.is_none()
        {
            return Ok(reset_remote_state(workspace));
        }

        let Some(_guard) = self.coordinator.try_enter(&workspace.id) else {
            return Err(RemoteWorkspaceError::ProvisioningAlreadyRunning {
                workspace_id: workspace.id.clone(),
            });
        };

        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        match &remote.remote_provisioning.status {
            RemoteProvisioningStatus::NotStarted => {
                self.handle_not_started(workspace, remote, provider)
            }
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::StartingRemoteProvisioner,
            } => {
                self.handle_starting_provisioner(workspace, remote, provider, phase)
                    .await
            }
            RemoteProvisioningStatus::InProgress {
                phase:
                    phase @ RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::CleaningUp,
                    },
            } => {
                self.handle_cleaning_up_provisioner(workspace, remote, provider, phase)
                    .await
            }
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::RunningRemoteProvisioner { .. },
            } => {
                self.handle_running_provisioner(workspace, remote, provider, phase)
                    .await
            }
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::CreatingRemoteEndpoint,
            } => {
                self.handle_creating_endpoint(workspace, remote, provider, phase)
                    .await
            }
            RemoteProvisioningStatus::Completed | RemoteProvisioningStatus::Failed { .. } => {
                self.handle_terminal_status(workspace)
            }
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteVolume,
            } => {
                self.handle_creating_volume(workspace, remote, provider)
                    .await
            }
            RemoteProvisioningStatus::Cancelling { phase } => {
                self.handle_cancelling(workspace, remote, provider, phase.clone())
                    .await
            }
        }
    }

    fn handle_terminal_status(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        Ok(workspace.clone())
    }

    fn handle_not_started(
        &self,
        workspace: &Workspace,
        _remote: &RemoteWorkspace,
        _provider: &dyn RemoteWorkspaceProvider,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        Ok(with_status_and_resources(
            workspace,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteVolume,
            },
            0,
            |_| {},
        ))
    }

    async fn handle_creating_volume(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote_volume = match provider
            .create_volume(CreateVolumeParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                size_bytes: remote.remote_placement.volume_size_bytes,
                mount_path: "/workspace".to_string(),
            })
            .await
        {
            Ok(remote_volume) => remote_volume,
            Err(error) => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(RemoteProvisioningPhase::CreatingRemoteVolume),
                    error.into(),
                ));
            }
        };

        Ok(with_status_and_resources(
            workspace,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
            },
            25,
            |resources| {
                resources.remote_volume = Some(remote_volume);
            },
        ))
    }

    async fn handle_starting_provisioner(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
        phase: &RemoteProvisioningPhase,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote_volume = match remote.remote_resources.remote_volume.as_ref() {
            Some(remote_volume) => remote_volume,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    RemoteProvisioningError::InvalidProvisioningState {
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

        let remote_provisioner = match provider
            .start_provisioner(StartProvisionerParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                volume_id: remote_volume.id.clone(),
                provisioner_image_ref,
                mount_path: "/workspace".to_string(),
            })
            .await
        {
            Ok(remote_provisioner) => remote_provisioner,
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
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Pending,
                },
            },
            50,
            |resources| {
                resources.remote_provisioner = Some(remote_provisioner);
            },
        ))
    }

    async fn handle_cleaning_up_provisioner(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
        phase: &RemoteProvisioningPhase,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote_provisioner = match remote.remote_resources.remote_provisioner.as_ref() {
            Some(remote_provisioner) => remote_provisioner,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    RemoteProvisioningError::InvalidProvisioningState {
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
                provisioner_id: remote_provisioner.id.clone(),
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
            RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. }
        ) {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                RemoteProvisioningError::InvalidProvisioningState {
                    message: format!("cleanup requires finished provisioner status: {status:?}"),
                },
            ));
        }

        let termination_result = provider
            .terminate_provisioner(TerminateProvisionerParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: remote_provisioner.id.clone(),
            })
            .await;

        match (status, termination_result) {
            (RemoteProvisionerStatus::Succeeded, Ok(())) => Ok(with_status_and_resources(
                workspace,
                RemoteProvisioningStatus::InProgress {
                    phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
                },
                75,
                |resources| {
                    resources.remote_provisioner = None;
                },
            )),
            (RemoteProvisionerStatus::Succeeded, Err(error)) => Ok(with_provisioning_failure(
                workspace,
                Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                error.into(),
            )),
            (RemoteProvisionerStatus::Failed { code, message }, Ok(())) => {
                let mut workspace = with_provisioning_failure(
                    workspace,
                    Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::Failed { code, message },
                    }),
                    RemoteProvisioningError::ProvisionerWorkerFailed,
                );
                let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
                remote.remote_resources.remote_provisioner = None;
                Ok(workspace)
            }
            (RemoteProvisionerStatus::Failed { code, message }, Err(_)) => {
                Ok(with_provisioning_failure(
                    workspace,
                    Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::Failed { code, message },
                    }),
                    RemoteProvisioningError::ProvisionerWorkerFailed,
                ))
            }
            _ => unreachable!("cleanup-ready status was validated before termination"),
        }
    }

    async fn handle_running_provisioner(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
        phase: &RemoteProvisioningPhase,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote_provisioner = match remote.remote_resources.remote_provisioner.as_ref() {
            Some(remote_provisioner) => remote_provisioner,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    RemoteProvisioningError::InvalidProvisioningState {
                        message: "remote provisioner snapshot is required before status polling"
                            .to_string(),
                    },
                ));
            }
        };
        let status = match provider
            .get_provisioner_status(GetProvisionerStatusParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: remote_provisioner.id.clone(),
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
            RemoteProvisionerStatus::Pending | RemoteProvisionerStatus::Starting => 50,
            RemoteProvisionerStatus::Running => 60,
            RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. } => 75,
            RemoteProvisionerStatus::CleaningUp => 75,
        };
        let provisioning_status = match status {
            RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. } => {
                RemoteProvisioningStatus::InProgress {
                    phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::CleaningUp,
                    },
                }
            }
            status => RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner { status },
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
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
        phase: &RemoteProvisioningPhase,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote_volume = match remote.remote_resources.remote_volume.as_ref() {
            Some(remote_volume) => remote_volume,
            None => {
                return Ok(with_provisioning_failure(
                    workspace,
                    Some(phase.clone()),
                    RemoteProvisioningError::InvalidProvisioningState {
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
        let remote_endpoint = match provider
            .create_endpoint(CreateEndpointParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: remote.remote_placement.datacenter_id.clone(),
                gpu_id: remote.remote_placement.gpu_id.clone(),
                volume_id: remote_volume.id.clone(),
                endpoint_image_ref,
                mount_path: "/workspace".to_string(),
                keep_alive_limits: remote.remote_placement.keep_alive_limits.clone(),
            })
            .await
        {
            Ok(remote_endpoint) => remote_endpoint,
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
            RemoteProvisioningStatus::Completed,
            100,
            |resources| {
                resources.remote_endpoint = Some(remote_endpoint);
            },
        ))
    }

    async fn handle_cancelling(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
        provider: &dyn RemoteWorkspaceProvider,
        phase: Option<RemoteProvisioningPhase>,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        if let Some(endpoint) = remote.remote_resources.remote_endpoint.as_ref() {
            return match ignore_expected_error(
                provider
                    .delete_endpoint(DeleteEndpointParams {
                        workspace_id: workspace.id.clone(),
                        endpoint_id: endpoint.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteEndpointNotFound,
            ) {
                Ok(()) => Ok(with_status_and_resources(
                    workspace,
                    RemoteProvisioningStatus::Cancelling {
                        phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                            status: RemoteProvisionerStatus::CleaningUp,
                        }),
                    },
                    75,
                    |resources| {
                        resources.remote_endpoint = None;
                    },
                )),
                Err(error) => Ok(with_cleanup_failure(
                    workspace,
                    Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                    error,
                )),
            };
        }

        if let Some(provisioner) = remote.remote_resources.remote_provisioner.as_ref() {
            let attempted_phase = match phase {
                Some(RemoteProvisioningPhase::RunningRemoteProvisioner { status }) => {
                    Some(RemoteProvisioningPhase::RunningRemoteProvisioner { status })
                }
                _ => Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
            };
            return match ignore_expected_error(
                provider
                    .terminate_provisioner(TerminateProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        provisioner_id: provisioner.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteProvisionerNotFound,
            ) {
                Ok(()) => Ok(with_status_and_resources(
                    workspace,
                    RemoteProvisioningStatus::Cancelling {
                        phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                    },
                    25,
                    |resources| {
                        resources.remote_provisioner = None;
                    },
                )),
                Err(error) => Ok(with_cleanup_failure(workspace, attempted_phase, error)),
            };
        }

        if let Some(volume) = remote.remote_resources.remote_volume.as_ref() {
            return match ignore_expected_error(
                provider
                    .delete_volume(DeleteVolumeParams {
                        workspace_id: workspace.id.clone(),
                        volume_id: volume.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteVolumeNotFound,
            ) {
                Ok(()) => Ok(reset_remote_state(workspace)),
                Err(error) => Ok(with_cleanup_failure(
                    workspace,
                    Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                    error,
                )),
            };
        }

        Ok(reset_remote_state(workspace))
    }

    pub fn cancel_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_runtime(workspace)?;

        let RemoteProvisioningStatus::InProgress { phase } = &remote.remote_provisioning.status
        else {
            return Ok(with_provisioning_failure(
                workspace,
                None,
                RemoteProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            ));
        };

        let mut workspace = workspace.clone();
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(phase.clone()),
        };
        Ok(workspace)
    }

    fn resolve_provisioner_image_ref(
        &self,
        workspace: &Workspace,
        remote: &RemoteWorkspace,
    ) -> Result<String, RemoteProvisioningError> {
        let contract =
            self.resolve_runtime_contract_reference(workspace, remote, |requirements| {
                &requirements.provisioner_contract
            })?;
        let catalog = self
            .workflow_catalog_service
            .get_provisioner_contract_catalog()
            .map_err(|error| RemoteProvisioningError::InvalidProvisioningState {
                message: format!("provisioner contract catalog is invalid: {error:?}"),
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            RemoteProvisioningError::InvalidProvisioningState {
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
        remote: &RemoteWorkspace,
    ) -> Result<String, RemoteProvisioningError> {
        let contract =
            self.resolve_runtime_contract_reference(workspace, remote, |requirements| {
                &requirements.endpoint_contract
            })?;
        let catalog = self
            .workflow_catalog_service
            .get_endpoint_contract_catalog()
            .map_err(|error| RemoteProvisioningError::InvalidProvisioningState {
                message: format!("endpoint contract catalog is invalid: {error:?}"),
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            RemoteProvisioningError::InvalidProvisioningState {
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
        remote: &RemoteWorkspace,
        contract: impl FnOnce(
            &'a crate::domain::workflow_preset::RemoteProviderRuntimeRequirements,
        ) -> &'a RuntimeContractReference,
    ) -> Result<&'a RuntimeContractReference, RemoteProvisioningError> {
        let provider_requirements = workspace
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(remote.remote_placement.gpu_cloud_provider_id)
            .ok_or_else(|| RemoteProvisioningError::InvalidProvisioningState {
                message: format!(
                    "workflow preset has no runtime requirements for provider {:?}",
                    remote.remote_placement.gpu_cloud_provider_id
                ),
            })?;

        Ok(contract(provider_requirements))
    }

    pub fn execute_workspace(&self, workspace: &Workspace) -> Result<(), RemoteWorkspaceError> {
        let remote = remote_runtime(workspace)?;

        if remote.remote_provisioning.status != RemoteProvisioningStatus::Completed {
            return Err(RemoteWorkspaceError::ExecuteWorkspaceNotReady);
        }

        if remote.remote_resources.remote_endpoint.is_none() {
            return Err(RemoteWorkspaceError::ExecuteWorkspaceMissingEndpoint);
        }

        Err(RemoteWorkspaceError::ExecuteWorkspaceNotImplemented {
            message: "endpoint worker execution is not implemented in this skeleton".to_string(),
        })
    }

    pub async fn cleanup_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_runtime(workspace)?;
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        let endpoint_cleanup = match &remote.remote_resources.remote_endpoint {
            Some(endpoint) => failed_workspace_from_result(
                workspace,
                provider
                    .delete_endpoint(DeleteEndpointParams {
                        workspace_id: workspace.id.clone(),
                        endpoint_id: endpoint.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteEndpointNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = endpoint_cleanup {
            return Ok(failed_workspace);
        }

        let provisioner_cleanup = match &remote.remote_resources.remote_provisioner {
            Some(provisioner) => failed_workspace_from_result(
                workspace,
                provider
                    .terminate_provisioner(TerminateProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        provisioner_id: provisioner.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteProvisionerNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = provisioner_cleanup {
            return Ok(failed_workspace);
        }

        let volume_cleanup = match &remote.remote_resources.remote_volume {
            Some(volume) => failed_workspace_from_result(
                workspace,
                provider
                    .delete_volume(DeleteVolumeParams {
                        workspace_id: workspace.id.clone(),
                        volume_id: volume.id.clone(),
                    })
                    .await,
                RemoteWorkspaceError::RemoteVolumeNotFound,
            ),
            None => None,
        };

        if let Some(failed_workspace) = volume_cleanup {
            return Ok(failed_workspace);
        }

        Ok(reset_remote_state(workspace))
    }
}

#[derive(Debug, Clone, Default)]
struct RemoteWorkspaceProvisioningCoordinator {
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl RemoteWorkspaceProvisioningCoordinator {
    fn try_enter(&self, workspace_id: &str) -> Option<RemoteWorkspaceProvisioningGuard> {
        let mut active = self
            .active_workspace_ids
            .lock()
            .expect("remote workspace provisioning coordinator lock");
        if !active.insert(workspace_id.to_string()) {
            return None;
        }

        Some(RemoteWorkspaceProvisioningGuard {
            workspace_id: workspace_id.to_string(),
            active_workspace_ids: Arc::clone(&self.active_workspace_ids),
        })
    }
}

struct RemoteWorkspaceProvisioningGuard {
    workspace_id: String,
    active_workspace_ids: Arc<Mutex<HashSet<String>>>,
}

impl Drop for RemoteWorkspaceProvisioningGuard {
    fn drop(&mut self) {
        self.active_workspace_ids
            .lock()
            .expect("remote workspace provisioning coordinator lock")
            .remove(&self.workspace_id);
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
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteProvisioningError, RemoteProvisioningPhase, RemoteVolumeSnapshot,
            RemoteWorkspaceResources, WorkspaceRuntime,
        },
    };

    use super::*;
    use crate::remote_workspace::{
        errors::RemoteWorkspaceError,
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
            RemoteProvisionerProvider, RemoteVolumeProvider, RemoteWorkspaceProvider,
            StartProvisionerParams, TerminateProvisionerParams,
        },
    };
    use crate::shared::AppFuture;

    #[derive(Default)]
    struct ProviderState {
        calls: Vec<&'static str>,
        placement_options_result: Option<Result<RemotePlacementOptions, RemoteWorkspaceError>>,
        create_volume_error: Option<RemoteWorkspaceError>,
        create_endpoint_error: Option<RemoteWorkspaceError>,
        start_provisioner_error: Option<RemoteWorkspaceError>,
        delete_endpoint_error: Option<RemoteWorkspaceError>,
        terminate_provisioner_error: Option<RemoteWorkspaceError>,
        delete_volume_error: Option<RemoteWorkspaceError>,
        provisioner_status_results: Vec<Result<RemoteProvisionerStatus, RemoteWorkspaceError>>,
        last_create_volume_params: Option<CreateVolumeParams>,
        last_create_endpoint_params: Option<CreateEndpointParams>,
        last_start_provisioner_params: Option<StartProvisionerParams>,
        last_get_provisioner_status_params: Option<GetProvisionerStatusParams>,
    }

    fn provider_request_failed(message: &str) -> RemoteWorkspaceError {
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

    impl RemotePlacementOptionsProvider for FakeProvider {
        fn get_provider_placement_options<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
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

    impl RemoteVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            params: CreateVolumeParams,
        ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("create_volume");
                state.last_create_volume_params = Some(params);

                if let Some(error) = state.create_volume_error.clone() {
                    return Err(error);
                }

                Ok(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
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

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            params: StartProvisionerParams,
        ) -> AppFuture<'a, Result<RemoteProvisionerSnapshot, RemoteWorkspaceError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("start_provisioner");
                state.last_start_provisioner_params = Some(params);

                if let Some(error) = state.start_provisioner_error.clone() {
                    return Err(error);
                }

                Ok(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
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
        ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("get_provisioner_status");
                state.last_get_provisioner_status_params = Some(params);
                if state.provisioner_status_results.is_empty() {
                    return Ok(RemoteProvisionerStatus::Pending);
                }
                state.provisioner_status_results.remove(0)
            })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            params: CreateEndpointParams,
        ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("create_endpoint");
                state.last_create_endpoint_params = Some(params);
                if let Some(error) = state.create_endpoint_error.clone() {
                    return Err(error);
                }
                Ok(RemoteEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _params: DeleteEndpointParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
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

    impl RemoteWorkspaceProvider for FakeProvider {
        fn provider_id(&self) -> GpuCloudProviderId {
            GpuCloudProviderId::Runpod
        }
    }

    fn service_with_state(state: Arc<Mutex<ProviderState>>) -> RemoteWorkspaceService {
        RemoteWorkspaceService::new(
            RemoteWorkspaceProviderRegistry::new(vec![Box::new(FakeProvider::new(state))]),
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

    fn draft_workspace(service: &RemoteWorkspaceService) -> Workspace {
        service
            .setup_workspace(SetupWorkspaceRequest {
                workspace_id: "workspace".to_string(),
                workflow_preset: workflow_preset(),
                remote_placement: placement_plan(),
            })
            .expect("workspace setup should succeed")
    }

    fn workspace_with_all_remote_resources(service: &RemoteWorkspaceService) -> Workspace {
        let mut workspace = draft_workspace(service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_resources.remote_endpoint = Some(RemoteEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        });
        workspace
    }

    #[test]
    fn setup_workspace_returns_remote_runtime_with_not_started_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));

        let workspace = draft_workspace(&service);

        let WorkspaceRuntime::Remote(remote) = workspace.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
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
        let service = RemoteWorkspaceService::new(
            RemoteWorkspaceProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );

        let error = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
            .expect_err("missing provider should fail");

        assert_eq!(
            error,
            RemoteWorkspaceError::ProviderUnavailable {
                provider_id: GpuCloudProviderId::Runpod
            }
        );
    }

    #[test]
    fn remote_provisioning_error_has_cancellation_cleanup_failed_variant() {
        assert_eq!(
            RemoteProvisioningError::CancellationCleanupFailed,
            RemoteProvisioningError::CancellationCleanupFailed
        );
    }

    #[test]
    fn cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        };
        remote.remote_provisioning.percent = Some(25);

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("in-progress workspace should enter cancellation");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(25));
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
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

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: None,
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;
        remote.remote_provisioning.percent = Some(100);

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("invalid cancellation should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: None,
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
            error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                message: "provider request failed".to_string(),
            }),
        };

        let cancelled = service
            .cancel_workspace(&workspace)
            .expect("invalid cancellation should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: None,
                error: RemoteProvisioningError::InvalidProvisioningState {
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

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(remote.remote_resources.remote_volume, None);
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(remote.remote_resources.remote_endpoint, None);
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteVolume
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(0));
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteVolume,
        };
        remote.remote_provisioning.percent = Some(0);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("creating volume workspace should create a volume");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string()
            })
        );
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(remote.remote_resources.remote_endpoint, None);
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::StartingRemoteProvisioner
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(25));
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteVolume,
        };
        remote.remote_provisioning.percent = Some(0);

        let guard = service
            .coordinator
            .try_enter(&workspace.id)
            .expect("first workspace entry should be accepted");
        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("duplicate workspace provisioning should be rejected");

        assert_eq!(
            error,
            RemoteWorkspaceError::ProvisioningAlreadyRunning {
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
        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
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
        let service = RemoteWorkspaceService::new(
            RemoteWorkspaceProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );
        let workspace = draft_workspace(&service);

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("missing provider should return service error");

        assert_eq!(
            error,
            RemoteWorkspaceError::ProviderUnavailable {
                provider_id: GpuCloudProviderId::Runpod,
            }
        );
    }

    #[test]
    fn provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.remote_provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should delete endpoint");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(remote.remote_resources.remote_endpoint, None);
        assert_eq!(
            remote.remote_resources.remote_provisioner,
            Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                })
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(75));
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.remote_provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("missing endpoint should skip to provisioner cleanup");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(remote.remote_resources.remote_endpoint, None);
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
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
            delete_endpoint_error: Some(RemoteWorkspaceError::RemoteEndpointNotFound),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.remote_provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("endpoint not found should be treated as already deleted");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(remote.remote_resources.remote_endpoint, None);
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                })
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            }),
        };
        remote.remote_provisioning.percent = Some(60);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should terminate provisioner");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Cancelling {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(25));
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
        };
        remote.remote_provisioning.percent = Some(25);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cancellation should delete volume");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };
        remote.remote_provisioning.percent = Some(75);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: Some(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                }),
                remote_provisioner: Some(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                }),
                remote_endpoint: Some(RemoteEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                }),
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            }),
        };
        remote.remote_provisioning.percent = Some(60);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources.remote_provisioner,
            Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Running,
                }),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
        };
        remote.remote_provisioning.percent = Some(25);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
            delete_endpoint_error: Some(RemoteWorkspaceError::RemoteVolumeNotFound),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = workspace_with_all_remote_resources(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
        };

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("cleanup failure should be represented in workspace state");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                error: RemoteProvisioningError::CancellationCleanupFailed,
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
        };
        remote.remote_provisioning.percent = Some(10);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("empty cancellation should reset workspace");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
        assert!(state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn provision_workspace_cancelling_without_resources_resets_without_provider_lookup() {
        let service = RemoteWorkspaceService::new(
            RemoteWorkspaceProviderRegistry::empty(),
            WorkflowCatalogService::new(),
        );
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
        };
        remote.remote_provisioning.percent = Some(10);

        let cancelled = block_on(service.provision_workspace(&workspace))
            .expect("empty cancellation should reset without provider lookup");

        let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
    }

    #[test]
    fn provision_workspace_starting_provisioner_advances_one_step() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        };
        remote.remote_provisioning.percent = Some(25);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("starting provisioner phase should start provisioner");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_provisioner,
            Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Pending
                }
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(50));
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
            })
        );
    }

    #[test]
    fn cleaning_up_status_is_plain_provisioner_status() {
        let status = RemoteProvisionerStatus::CleaningUp;

        assert_eq!(status, RemoteProvisionerStatus::CleaningUp);
    }

    #[test]
    fn provision_workspace_running_provisioner_stores_incomplete_status() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Running)],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Pending,
            },
        };
        remote.remote_provisioning.percent = Some(50);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("running provisioner should poll status");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Running
                }
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(60));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status"]
        );
    }

    #[test]
    fn provision_workspace_worker_success_moves_to_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Succeeded)],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("worker success should move to cleanup");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp
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
        let failed_status = RemoteProvisionerStatus::Failed {
            code: "provisioner_worker_asset_download_failed".to_string(),
            message: "asset download failed".to_string(),
        };
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(failed_status.clone())],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("worker failure should move to cleanup before failed state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp
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
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Succeeded)],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup after success should terminate provisioner");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteEndpoint
            }
        );
        assert_eq!(remote.remote_provisioning.percent, Some(75));
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["get_provisioner_status", "terminate_provisioner"]
        );
    }

    #[test]
    fn provision_workspace_cleanup_after_worker_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Failed {
                code: "provisioner_worker_step_timeout".to_string(),
                message: "step timed out".to_string(),
            })],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup after worker failure should mark failed");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(remote.remote_resources.remote_provisioner, None);
        assert_eq!(
            remote.remote_resources.remote_volume,
            Some(RemoteVolumeSnapshot {
                id: "volume".to_string()
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Failed {
                        code: "provisioner_worker_step_timeout".to_string(),
                        message: "step timed out".to_string(),
                    },
                }),
                error: RemoteProvisioningError::ProvisionerWorkerFailed,
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_error_after_success_marks_failed_and_preserves_provisioner() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Succeeded)],
            terminate_provisioner_error: Some(provider_request_failed("terminate failed")),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup error after success should become failed workspace");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_provisioner,
            Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "terminate failed".to_string(),
                }),
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_error_after_worker_failure_preserves_worker_failure() {
        let failed_status = RemoteProvisionerStatus::Failed {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("cleanup error after worker failure should preserve worker failure");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_provisioner,
            Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: failed_status,
                }),
                error: RemoteProvisioningError::ProvisionerWorkerFailed,
            }
        );
    }

    #[test]
    fn provision_workspace_cleanup_with_incomplete_status_returns_invalid_state_without_termination(
    ) {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Running)],
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("incomplete cleanup status should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteVolume,
        };
        remote.remote_provisioning.percent = Some(0);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("provider request failure should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("start provisioner failure should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "start failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("status poll failure should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Running,
                }),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "status failed".to_string(),
                }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("create endpoint failure should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                    message: "endpoint failed".to_string(),
                }),
            }
        );
    }

    #[test]
    fn provision_workspace_completed_returns_workspace_unchanged_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;
        remote.remote_provisioning.percent = Some(100);

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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
            error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                message: "raw failure".to_string(),
            }),
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("endpoint creation should complete workspace");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_resources.remote_endpoint,
            Some(RemoteEndpointSnapshot {
                id: "endpoint".to_string(),
                url: "https://endpoint.example".to_string(),
            })
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Completed
        );
        assert_eq!(remote.remote_provisioning.percent, Some(100));
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing volume should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing volume should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing provisioner should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Running,
                }),
                error: RemoteProvisioningError::InvalidProvisioningState {
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            },
        };

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("missing provisioner should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                error: RemoteProvisioningError::InvalidProvisioningState {
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

        assert_eq!(error, RemoteWorkspaceError::ExecuteWorkspaceNotReady);
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;

        let error = service
            .execute_workspace(&workspace)
            .expect_err("completed workspace without endpoint should not execute");

        assert_eq!(error, RemoteWorkspaceError::ExecuteWorkspaceMissingEndpoint);
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;
        remote.remote_resources.remote_endpoint = Some(RemoteEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        });

        let error = service
            .execute_workspace(&workspace)
            .expect_err("endpoint execution is not implemented yet");

        assert_eq!(
            error,
            RemoteWorkspaceError::ExecuteWorkspaceNotImplemented {
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
        let WorkspaceRuntime::Remote(remote) = cleaned_workspace.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
    }

    #[test]
    fn cleanup_workspace_ignores_not_found_cleanup_errors() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(RemoteWorkspaceError::RemoteEndpointNotFound),
            terminate_provisioner_error: Some(RemoteWorkspaceError::RemoteProvisionerNotFound),
            delete_volume_error: Some(RemoteWorkspaceError::RemoteVolumeNotFound),
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
        let WorkspaceRuntime::Remote(remote) = cleaned_workspace.runtime;
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::NotStarted
        );
        assert_eq!(remote.remote_provisioning.percent, None);
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
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
            phase: None,
            error: RemoteProvisioningError::Provider(ProviderApiError::RequestFailed {
                message: message.to_string(),
            }),
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
