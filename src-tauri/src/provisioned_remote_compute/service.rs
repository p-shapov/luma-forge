use crate::domain::{
    placement::{RemotePlacementOptions, RemotePlacementPlan},
    provider::GpuCloudProviderId,
    workflow_preset::WorkflowPreset,
    workspace::{
        ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningPhase,
        ProvisionedRemoteComputeProvisioningState, ProvisionedRemoteComputeProvisioningStatus,
        ProvisionedRemoteComputeResources, ProvisionedRemoteComputeWorkspace, Workspace,
        WorkspaceRuntime,
    },
};
use crate::workflow_catalog::WorkflowCatalogService;

use super::{
    cleanup,
    coordination::ProvisionedRemoteComputeCoordinator,
    errors::ProvisionedRemoteComputeError,
    flow::{self, ProvisionedRemoteComputeFlowContext},
    helpers::{remote_runtime, reset_remote_state},
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
            return flow::handle_terminal_status(workspace);
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
        let flow_context = ProvisionedRemoteComputeFlowContext {
            workflow_catalog_service: &self.workflow_catalog_service,
            provider,
        };

        match &remote.provisioning.status {
            ProvisionedRemoteComputeProvisioningStatus::NotStarted => {
                flow::handle_not_started(workspace)
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
            } => flow::handle_creating_volume(workspace, remote, &flow_context).await,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: phase @ ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
            } => flow::handle_starting_provisioner(workspace, remote, &flow_context, phase).await,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase:
                    phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    },
            } => {
                flow::handle_cleaning_up_provisioner(workspace, remote, &flow_context, phase).await
            }
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase:
                    phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        ..
                    },
            } => flow::handle_running_provisioner(workspace, remote, &flow_context, phase).await,
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: phase @ ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
            } => flow::handle_creating_endpoint(workspace, remote, &flow_context, phase).await,
            ProvisionedRemoteComputeProvisioningStatus::Completed
            | ProvisionedRemoteComputeProvisioningStatus::Failed { .. } => {
                flow::handle_terminal_status(workspace)
            }
            ProvisionedRemoteComputeProvisioningStatus::Cancelling { phase } => {
                cleanup::handle_cancelling(workspace, remote, provider, phase.clone()).await
            }
        }
    }

    pub fn cancel_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, ProvisionedRemoteComputeError> {
        cleanup::mark_cancelling(workspace)
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

        cleanup::cleanup_workspace(workspace, remote, provider).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        provider::{GpuCloudProviderId, ProviderApiError},
        workspace::{
            ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeProvisioningError,
            ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
            ProvisionedRemoteComputeResources, ProvisionedRemoteComputeVolumeSnapshot,
            WorkspaceRuntime,
        },
    };
    use crate::provisioned_remote_compute::{
        errors::ProvisionedRemoteComputeError,
        registry::ProvisionedRemoteComputeProviderRegistry,
        test_support::{
            block_on, draft_workspace, placement_options, service_with_state, ProviderState,
        },
    };
    use crate::workflow_catalog::WorkflowCatalogService;

    use super::*;

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
}
