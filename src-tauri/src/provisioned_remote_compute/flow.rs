use crate::domain::workspace::{
    ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
    ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
    ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
};
use crate::workflow_catalog::WorkflowCatalogService;

use super::{
    contracts::ProvisionedRemoteComputeContractResolver,
    errors::ProvisionedRemoteComputeError,
    helpers::{with_provisioning_failure, with_status_and_resources},
    provider::{
        CreateEndpointParams, CreateVolumeParams, GetProvisionerStatusParams,
        ProvisionedRemoteComputeProvider, StartProvisionerParams, TerminateProvisionerParams,
    },
};

pub(crate) struct ProvisionedRemoteComputeFlowContext<'a> {
    pub(crate) workflow_catalog_service: &'a WorkflowCatalogService,
    pub(crate) provider: &'a dyn ProvisionedRemoteComputeProvider,
}

pub(crate) fn handle_terminal_status(
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    Ok(workspace.clone())
}

pub(crate) fn handle_not_started(
    workspace: &Workspace,
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

pub(crate) async fn handle_creating_volume(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let volume = match context
        .provider
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

pub(crate) async fn handle_starting_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
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
    let resolver = ProvisionedRemoteComputeContractResolver::new(context.workflow_catalog_service);
    let provisioner_image_ref = match resolver.provisioner_image_ref(workspace, remote) {
        Ok(image_ref) => image_ref,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error,
            ));
        }
    };

    let provisioner = match context
        .provider
        .start_provisioner(StartProvisionerParams {
            workspace_id: workspace.id.clone(),
            datacenter_id: remote.remote_placement.datacenter_id.clone(),
            gpu_id: remote.remote_placement.gpu_id.clone(),
            volume_id: volume.id.clone(),
            provisioner_image_ref,
            mount_path: "/workspace".to_string(),
            requires_hugging_face_api_key: workspace.workflow_preset.requires_hugging_face_api_key,
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

pub(crate) async fn handle_cleaning_up_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
    phase: &ProvisionedRemoteComputeProvisioningPhase,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let provisioner = match remote.resources.provisioner.as_ref() {
        Some(provisioner) => provisioner,
        None => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote provisioner snapshot is required before provisioner cleanup"
                        .to_string(),
                },
            ));
        }
    };
    let status = match context
        .provider
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

    let termination_result = context
        .provider
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
                        status: ProvisionedRemoteComputeProvisionerStatus::Failed { code, message },
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
                        status: ProvisionedRemoteComputeProvisionerStatus::Failed { code, message },
                    },
                ),
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
            ))
        }
        _ => unreachable!("cleanup-ready status was validated before termination"),
    }
}

pub(crate) async fn handle_running_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
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
    let status = match context
        .provider
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
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status },
        },
    };

    Ok(with_status_and_resources(
        workspace,
        provisioning_status,
        percent,
        |_| {},
    ))
}

pub(crate) async fn handle_creating_endpoint(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
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
    let resolver = ProvisionedRemoteComputeContractResolver::new(context.workflow_catalog_service);
    let endpoint_image_ref = match resolver.endpoint_image_ref(workspace, remote) {
        Ok(image_ref) => image_ref,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error,
            ));
        }
    };
    let endpoint = match context
        .provider
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        placement::RemoteEndpointKeepAliveLimits,
        provider::ProviderApiError,
        workspace::{
            ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeProvisionerSnapshot,
            ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
            ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
            ProvisionedRemoteComputeVolumeSnapshot, WorkspaceRuntime,
        },
    };
    use crate::provisioned_remote_compute::{
        errors::ProvisionedRemoteComputeError,
        provider::{CreateEndpointParams, CreateVolumeParams, StartProvisionerParams},
        test_support::{
            block_on, draft_workspace, provider_request_failed, service_with_state, ProviderState,
        },
    };

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
}
