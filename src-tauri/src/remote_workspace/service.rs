use crate::domain::{
    placement::RemotePlacementPlan,
    workflow_preset::WorkflowPreset,
    workspace::{
        RemoteProvisionerStatus, RemoteProvisioningError, RemoteProvisioningPhase,
        RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
        RemoteWorkspaceResources, Workspace, WorkspaceRuntime,
    },
};

use super::{
    errors::RemoteWorkspaceError,
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, RemoteWorkspaceProvider, StartProvisionerParams,
        TerminateProvisionerParams,
    },
    registry::RemoteWorkspaceProviderRegistry,
};

const UNRESOLVED_PROVISIONER_IMAGE_REF: &str = "unresolved-provisioner-image";
const UNRESOLVED_ENDPOINT_IMAGE_REF: &str = "unresolved-endpoint-image";

pub struct SetupWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset: WorkflowPreset,
    pub remote_placement: RemotePlacementPlan,
}

pub struct RemoteWorkspaceService {
    provider_registry: RemoteWorkspaceProviderRegistry,
}

impl RemoteWorkspaceService {
    pub fn new(provider_registry: RemoteWorkspaceProviderRegistry) -> Self {
        Self { provider_registry }
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

    pub async fn provision_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;

        if matches!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Completed | RemoteProvisioningStatus::Failed { .. }
        ) {
            return Ok(workspace.clone());
        }

        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        match &remote.remote_provisioning.status {
            RemoteProvisioningStatus::NotStarted => {
                let remote_volume = match provider
                    .create_volume(CreateVolumeParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        size_bytes: remote.remote_placement.remote_volume_size_bytes,
                        mount_path: "/workspace".to_string(),
                    })
                    .await
                {
                    Ok(remote_volume) => remote_volume,
                    Err(error) => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(RemoteProvisioningPhase::CreatingRemoteVolume),
                            error.into(),
                        ));
                    }
                };

                Ok(update_provisioning_workspace(
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
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::StartingRemoteProvisioner,
            } => {
                let remote_volume = match remote.remote_resources.remote_volume.as_ref() {
                    Some(remote_volume) => remote_volume,
                    None => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            RemoteProvisioningError::InvalidProvisioningState {
                                message:
                                    "remote volume snapshot is required before provisioner start"
                                        .to_string(),
                            },
                        ));
                    }
                };

                let remote_provisioner = match provider
                    .start_provisioner(StartProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        volume_id: remote_volume.id.clone(),
                        provisioner_image_ref: UNRESOLVED_PROVISIONER_IMAGE_REF.to_string(),
                        mount_path: "/workspace".to_string(),
                    })
                    .await
                {
                    Ok(remote_provisioner) => remote_provisioner,
                    Err(error) => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            error.into(),
                        ));
                    }
                };

                Ok(update_provisioning_workspace(
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
            RemoteProvisioningStatus::InProgress {
                phase:
                    phase @ RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::CleaningUp,
                    },
            } => {
                let remote_provisioner = match remote.remote_resources.remote_provisioner.as_ref() {
                    Some(remote_provisioner) => remote_provisioner,
                    None => {
                        return Ok(failed_provisioning_workspace(
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
                        return Ok(failed_provisioning_workspace(
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
                    return Ok(failed_provisioning_workspace(
                        workspace,
                        Some(phase.clone()),
                        RemoteProvisioningError::InvalidProvisioningState {
                            message: format!(
                                "cleanup requires finished provisioner status: {status:?}"
                            ),
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
                    (RemoteProvisionerStatus::Succeeded, Ok(())) => {
                        Ok(update_provisioning_workspace(
                            workspace,
                            RemoteProvisioningStatus::InProgress {
                                phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
                            },
                            75,
                            |resources| {
                                resources.remote_provisioner = None;
                            },
                        ))
                    }
                    (RemoteProvisionerStatus::Succeeded, Err(error)) => {
                        Ok(failed_provisioning_workspace(
                            workspace,
                            Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                                status: RemoteProvisionerStatus::CleaningUp,
                            }),
                            error.into(),
                        ))
                    }
                    (RemoteProvisionerStatus::Failed { code, message }, Ok(())) => {
                        let mut workspace = failed_provisioning_workspace(
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
                        Ok(failed_provisioning_workspace(
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
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::RunningRemoteProvisioner { .. },
            } => {
                let remote_provisioner = match remote.remote_resources.remote_provisioner.as_ref() {
                    Some(remote_provisioner) => remote_provisioner,
                    None => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            RemoteProvisioningError::InvalidProvisioningState {
                                message:
                                    "remote provisioner snapshot is required before status polling"
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
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            error.into(),
                        ));
                    }
                };

                let percent = match &status {
                    RemoteProvisionerStatus::Pending | RemoteProvisionerStatus::Starting => 50,
                    RemoteProvisionerStatus::Running => 60,
                    RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. } => {
                        75
                    }
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

                Ok(update_provisioning_workspace(
                    workspace,
                    provisioning_status,
                    percent,
                    |_| {},
                ))
            }
            RemoteProvisioningStatus::InProgress {
                phase: phase @ RemoteProvisioningPhase::CreatingRemoteEndpoint,
            } => {
                let remote_volume = match remote.remote_resources.remote_volume.as_ref() {
                    Some(remote_volume) => remote_volume,
                    None => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            RemoteProvisioningError::InvalidProvisioningState {
                                message:
                                    "remote volume snapshot is required before endpoint creation"
                                        .to_string(),
                            },
                        ));
                    }
                };
                let remote_endpoint = match provider
                    .create_endpoint(CreateEndpointParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        volume_id: remote_volume.id.clone(),
                        endpoint_image_ref: UNRESOLVED_ENDPOINT_IMAGE_REF.to_string(),
                        mount_path: "/workspace".to_string(),
                    })
                    .await
                {
                    Ok(remote_endpoint) => remote_endpoint,
                    Err(error) => {
                        return Ok(failed_provisioning_workspace(
                            workspace,
                            Some(phase.clone()),
                            error.into(),
                        ));
                    }
                };

                Ok(update_provisioning_workspace(
                    workspace,
                    RemoteProvisioningStatus::Completed,
                    100,
                    |resources| {
                        resources.remote_endpoint = Some(remote_endpoint);
                    },
                ))
            }
            RemoteProvisioningStatus::Completed | RemoteProvisioningStatus::Failed { .. } => {
                unreachable!("terminal provisioning states are handled before provider lookup")
            }
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteVolume,
            } => Ok(failed_provisioning_workspace(
                workspace,
                Some(RemoteProvisioningPhase::CreatingRemoteVolume),
                RemoteProvisioningError::InvalidProvisioningState {
                    message: "remote volume creation starts from not started provisioning state"
                        .to_string(),
                },
            )),
            RemoteProvisioningStatus::Cancelling { phase } => {
                cancel_provisioning_step(workspace, remote, provider, phase.clone()).await
            }
        }
    }

    pub fn cancel_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;

        let RemoteProvisioningStatus::InProgress { phase } = &remote.remote_provisioning.status
        else {
            return Ok(failed_provisioning_workspace(
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

    pub fn execute_workspace(&self, workspace: &Workspace) -> Result<(), RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;

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
        let remote = remote_workspace(workspace)?;
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        let endpoint_cleanup = match &remote.remote_resources.remote_endpoint {
            Some(endpoint) => cleanup_failed_workspace(
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
            Some(provisioner) => cleanup_failed_workspace(
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
            Some(volume) => cleanup_failed_workspace(
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

        let mut workspace = workspace.clone();
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_resources = RemoteWorkspaceResources {
            remote_volume: None,
            remote_provisioner: None,
            remote_endpoint: None,
        };
        remote.remote_provisioning.status = RemoteProvisioningStatus::NotStarted;
        remote.remote_provisioning.percent = None;
        Ok(workspace)
    }
}

async fn cancel_provisioning_step(
    workspace: &Workspace,
    remote: &RemoteWorkspace,
    provider: &dyn RemoteWorkspaceProvider,
    phase: Option<RemoteProvisioningPhase>,
) -> Result<Workspace, RemoteWorkspaceError> {
    if let Some(endpoint) = remote.remote_resources.remote_endpoint.as_ref() {
        return match ignore_cleanup_error(
            provider
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                })
                .await,
            RemoteWorkspaceError::RemoteEndpointNotFound,
        ) {
            Ok(()) => Ok(update_cancelling_workspace(
                workspace,
                Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                75,
                |resources| {
                    resources.remote_endpoint = None;
                },
            )),
            Err(error) => Ok(failed_cancellation_workspace(workspace, phase, error)),
        };
    }

    Ok(update_cancelling_workspace(workspace, None, 0, |_| {}))
}

fn cleanup_failed_workspace(
    workspace: &Workspace,
    result: Result<(), RemoteWorkspaceError>,
    ignored_error: RemoteWorkspaceError,
) -> Option<Workspace> {
    ignore_cleanup_error(result, ignored_error)
        .err()
        .map(|error| {
            failed_provisioning_workspace(workspace, None, RemoteProvisioningError::from(error))
        })
}

fn ignore_cleanup_error(
    result: Result<(), RemoteWorkspaceError>,
    ignored_error: RemoteWorkspaceError,
) -> Result<(), RemoteWorkspaceError> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error == ignored_error => Ok(()),
        Err(error) => Err(error),
    }
}

fn remote_workspace(workspace: &Workspace) -> Result<&RemoteWorkspace, RemoteWorkspaceError> {
    // When WorkspaceRuntime gets non-remote variants, return an explicit
    // RemoteWorkspaceError here instead of accepting them in this service.
    let WorkspaceRuntime::Remote(remote) = &workspace.runtime;
    Ok(remote)
}

fn failed_provisioning_workspace(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    error: RemoteProvisioningError,
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Failed { phase, error };
    workspace
}

fn failed_cancellation_workspace(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    error: RemoteWorkspaceError,
) -> Workspace {
    let provisioning_error = match error {
        RemoteWorkspaceError::Provider(error) => RemoteProvisioningError::Provider(error),
        _ => RemoteProvisioningError::CancellationCleanupFailed,
    };
    failed_provisioning_workspace(workspace, phase, provisioning_error)
}

fn update_cancelling_workspace(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    percent: u8,
    update_resources: impl FnOnce(&mut RemoteWorkspaceResources),
) -> Workspace {
    update_provisioning_workspace(
        workspace,
        RemoteProvisioningStatus::Cancelling { phase },
        percent,
        update_resources,
    )
}

fn update_provisioning_workspace(
    workspace: &Workspace,
    status: RemoteProvisioningStatus,
    percent: u8,
    update_resources: impl FnOnce(&mut RemoteWorkspaceResources),
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    update_resources(&mut remote.remote_resources);
    remote.remote_provisioning.status = status;
    remote.remote_provisioning.percent = Some(percent);
    workspace
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        placement::{
            Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
            RemotePlacementPlan,
        },
        provider::{GpuCloudProviderId, ProviderError},
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
            GetProvisionerStatusParams, RemoteEndpointProvider, RemoteProvisionerProvider,
            RemoteVolumeProvider, RemoteWorkspaceProvider, StartProvisionerParams,
            TerminateProvisionerParams,
        },
    };
    use crate::shared::AppFuture;

    #[derive(Default)]
    struct ProviderState {
        calls: Vec<&'static str>,
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

    struct FakeProvider {
        state: Arc<Mutex<ProviderState>>,
    }

    impl FakeProvider {
        fn new(state: Arc<Mutex<ProviderState>>) -> Self {
            Self { state }
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
        RemoteWorkspaceService::new(RemoteWorkspaceProviderRegistry::new(vec![Box::new(
            FakeProvider::new(state),
        )]))
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
                        id: "endpoint".to_string(),
                        version: "1".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner".to_string(),
                        version: "1".to_string(),
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
            remote_volume_size_bytes: 1,
            remote_capabilities: RemotePlacementCapabilities {
                remote_endpoint_keep_alive: Capability::Supported(RemoteEndpointKeepAliveLimits {
                    default_seconds: 60,
                    min_seconds: 30,
                    max_seconds: 120,
                }),
            },
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
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
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
    fn provision_workspace_not_started_creates_volume_only() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("not started workspace should create a volume");

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
    fn provision_workspace_missing_provider_returns_error_without_failed_workspace() {
        let service = RemoteWorkspaceService::new(RemoteWorkspaceProviderRegistry::empty());
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
                provisioner_image_ref: UNRESOLVED_PROVISIONER_IMAGE_REF.to_string(),
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
            terminate_provisioner_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "terminate failed".to_string(),
                },
            )),
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
                error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
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
            terminate_provisioner_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "terminate failed".to_string(),
                },
            )),
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
            create_volume_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "provider request failed".to_string(),
                },
            )),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let provisioned = block_on(service.provision_workspace(&workspace))
            .expect("provider request failure should fail provisioning state");

        let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
        assert_eq!(
            remote.remote_provisioning.status,
            RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
                error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                    message: "provider request failed".to_string(),
                }),
            }
        );
    }

    #[test]
    fn provision_workspace_start_provisioner_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            start_provisioner_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "start failed".to_string(),
                },
            )),
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
                error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                    message: "start failed".to_string(),
                }),
            }
        );
    }

    #[test]
    fn provision_workspace_status_poll_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![Err(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "status failed".to_string(),
                },
            ))],
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
                error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                    message: "status failed".to_string(),
                }),
            }
        );
    }

    #[test]
    fn provision_workspace_create_endpoint_failure_marks_failed() {
        let state = Arc::new(Mutex::new(ProviderState {
            create_endpoint_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "endpoint failed".to_string(),
                },
            )),
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
                error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
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
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
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
            delete_endpoint_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "provider request failed".to_string(),
                },
            )),
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
            terminate_provisioner_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "provider request failed".to_string(),
                },
            )),
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
            delete_volume_error: Some(RemoteWorkspaceError::Provider(
                ProviderError::RequestFailed {
                    message: "provider request failed".to_string(),
                },
            )),
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
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
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
