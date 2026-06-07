use crate::domain::workspace::{
    ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
    ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
    ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
};

use super::{
    errors::ProvisionedRemoteComputeError,
    helpers::{
        failed_workspace_from_result, ignore_expected_error, reset_remote_state,
        with_cleanup_failure, with_provisioning_failure, with_status_and_resources,
    },
    provider::{
        DeleteEndpointParams, DeleteVolumeParams, ProvisionedRemoteComputeProvider,
        TerminateProvisionerParams,
    },
};

pub(crate) async fn handle_cancelling(
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
            }) => {
                Some(ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status })
            }
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

pub(crate) fn mark_cancelling(
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &workspace.runtime;

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

pub(crate) async fn cleanup_workspace(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    provider: &dyn ProvisionedRemoteComputeProvider,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        provider::ProviderApiError,
        workspace::{
            ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeProvisionerSnapshot,
            ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
            ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
            ProvisionedRemoteComputeResources, ProvisionedRemoteComputeVolumeSnapshot,
            WorkspaceRuntime,
        },
    };
    use crate::provisioned_remote_compute::{
        errors::ProvisionedRemoteComputeError,
        registry::ProvisionedRemoteComputeProviderRegistry,
        service::ProvisionedRemoteComputeService,
        test_support::{
            block_on, draft_workspace, failed_cleanup_workspace, provider_request_failed,
            service_with_state, workspace_with_all_remote_resources, ProviderState,
        },
    };
    use crate::workflow_catalog::WorkflowCatalogService;

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
}
