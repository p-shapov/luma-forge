use crate::domain::{
    placement::RemotePlacementPlan,
    workflow_preset::WorkflowPreset,
    workspace::{
        RemoteProvisionerStatus, RemoteProvisioningPhase, RemoteProvisioningState,
        RemoteProvisioningStatus, RemoteWorkspace, RemoteWorkspaceResources, Workspace,
        WorkspaceRuntime,
    },
};

use super::{
    errors::RemoteWorkspaceError,
    provider::{
        CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams, StartProvisionerParams,
        TerminateProvisionerParams,
    },
    registry::RemoteWorkspaceProviderRegistry,
};

const UNRESOLVED_PROVISIONER_IMAGE_REF: &str = "unresolved-provisioner-image";

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
            return Err(RemoteWorkspaceError::InvalidRequest {
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

        match &remote.remote_provisioning.status {
            RemoteProvisioningStatus::NotStarted => {
                let provider_id = remote.remote_placement.gpu_cloud_provider_id;
                let provider = self.provider_registry.for_provider(provider_id)?;

                let remote_volume = provider
                    .create_volume(CreateVolumeParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        size_bytes: remote.remote_placement.remote_volume_size_bytes,
                        mount_path: "/workspace".to_string(),
                    })
                    .await?;

                let mut workspace = workspace.clone();
                let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
                remote.remote_resources.remote_volume = Some(remote_volume);
                remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
                    phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
                };
                remote.remote_provisioning.percent = Some(25);

                Ok(workspace)
            }
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
            } => {
                let remote_volume = remote.remote_resources.remote_volume.as_ref().ok_or(
                    RemoteWorkspaceError::InvalidWorkspaceState {
                        message: "remote volume snapshot is required before provisioner start"
                            .to_string(),
                    },
                )?;
                let provider_id = remote.remote_placement.gpu_cloud_provider_id;
                let provider = self.provider_registry.for_provider(provider_id)?;

                let remote_provisioner = provider
                    .start_provisioner(StartProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        volume_id: remote_volume.id.clone(),
                        provisioner_image_ref: UNRESOLVED_PROVISIONER_IMAGE_REF.to_string(),
                        mount_path: "/workspace".to_string(),
                    })
                    .await?;

                let mut workspace = workspace.clone();
                let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
                remote.remote_resources.remote_provisioner = Some(remote_provisioner);
                remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
                    phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                        status: RemoteProvisionerStatus::Pending,
                    },
                };
                remote.remote_provisioning.percent = Some(50);

                Ok(workspace)
            }
            RemoteProvisioningStatus::Completed => Ok(workspace.clone()),
            RemoteProvisioningStatus::Failed { .. } => {
                Err(RemoteWorkspaceError::InvalidWorkspaceState {
                    message:
                        "failed workspace must be deleted or reset before provisioning can continue"
                            .to_string(),
                })
            }
            _ => Err(RemoteWorkspaceError::NotImplemented {
                message: "provisioning step is not implemented in this skeleton".to_string(),
            }),
        }
    }

    pub fn execute_workspace(&self, workspace: &Workspace) -> Result<(), RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;

        if remote.remote_provisioning.status != RemoteProvisioningStatus::Completed {
            return Err(RemoteWorkspaceError::WorkspaceNotReady);
        }

        if remote.remote_resources.remote_endpoint.is_none() {
            return Err(RemoteWorkspaceError::MissingEndpoint);
        }

        Err(RemoteWorkspaceError::NotImplemented {
            message: "endpoint worker execution is not implemented in this skeleton".to_string(),
        })
    }

    pub async fn delete_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<(), RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self.provider_registry.for_provider(provider_id)?;

        if let Some(endpoint) = &remote.remote_resources.remote_endpoint {
            if let Err(error) = provider
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                })
                .await
            {
                if error != RemoteWorkspaceError::NonExistingEndpoint {
                    return Err(error);
                }
            }
        }

        if let Some(provisioner) = &remote.remote_resources.remote_provisioner {
            if let Err(error) = provider
                .terminate_provisioner(TerminateProvisionerParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id.clone(),
                })
                .await
            {
                if error != RemoteWorkspaceError::NonExistingProvisioner {
                    return Err(error);
                }
            }
        }

        if let Some(volume) = &remote.remote_resources.remote_volume {
            if let Err(error) = provider
                .delete_volume(DeleteVolumeParams {
                    workspace_id: workspace.id.clone(),
                    volume_id: volume.id.clone(),
                })
                .await
            {
                if error != RemoteWorkspaceError::NonExistingVolume {
                    return Err(error);
                }
            }
        }

        Ok(())
    }
}

fn remote_workspace(workspace: &Workspace) -> Result<&RemoteWorkspace, RemoteWorkspaceError> {
    // When WorkspaceRuntime gets non-remote variants, return an explicit
    // RemoteWorkspaceError here instead of accepting them in this service.
    let WorkspaceRuntime::Remote(remote) = &workspace.runtime;
    Ok(remote)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        placement::{
            Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
            RemotePlacementPlan,
        },
        provider::GpuCloudProviderId,
        runtime_contract::RuntimeContractReference,
        workflow_preset::{
            RemoteProviderRuntimeRequirements, RemoteRuntimeRequirements, WorkflowExecutionType,
            WorkflowPreset,
        },
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteProvisioningPhase, RemoteVolumeSnapshot, RemoteWorkspaceResources,
            WorkspaceRuntime,
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
        start_provisioner_error: Option<RemoteWorkspaceError>,
        delete_endpoint_error: Option<RemoteWorkspaceError>,
        terminate_provisioner_error: Option<RemoteWorkspaceError>,
        delete_volume_error: Option<RemoteWorkspaceError>,
        last_create_volume_params: Option<CreateVolumeParams>,
        last_start_provisioner_params: Option<StartProvisionerParams>,
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
            _params: GetProvisionerStatusParams,
        ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
            Box::pin(async {
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
    fn provision_workspace_returns_provider_request_failed_messages() {
        let state = Arc::new(Mutex::new(ProviderState {
            create_volume_error: Some(RemoteWorkspaceError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
            }),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("provider request failure should be returned as a provisioning error");

        assert_eq!(
            error,
            RemoteWorkspaceError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
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
    fn provision_workspace_failed_returns_invalid_state_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
            code: "provider_error".to_string(),
            message: "raw failure".to_string(),
        };

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("failed workspace should not continue provisioning");

        assert_eq!(
            error,
            RemoteWorkspaceError::InvalidWorkspaceState {
                message:
                    "failed workspace must be deleted or reset before provisioning can continue"
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
    fn provision_workspace_unsupported_in_progress_phase_returns_not_implemented_without_provider_calls(
    ) {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let mut workspace = draft_workspace(&service);
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
        };

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("unsupported phase should not run provider calls");

        assert_eq!(
            error,
            RemoteWorkspaceError::NotImplemented {
                message: "provisioning step is not implemented in this skeleton".to_string(),
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

        let error = block_on(service.provision_workspace(&workspace))
            .expect_err("missing volume should stop provisioner start");

        assert_eq!(
            error,
            RemoteWorkspaceError::InvalidWorkspaceState {
                message: "remote volume snapshot is required before provisioner start".to_string(),
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

        assert_eq!(error, RemoteWorkspaceError::WorkspaceNotReady);
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

        assert_eq!(error, RemoteWorkspaceError::MissingEndpoint);
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
            RemoteWorkspaceError::NotImplemented {
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
    fn delete_workspace_cleans_resources_in_dependency_order() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        block_on(service.delete_workspace(&workspace)).expect("workspace cleanup should succeed");

        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
        );
    }

    #[test]
    fn delete_workspace_ignores_not_found_cleanup_errors() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(RemoteWorkspaceError::NonExistingEndpoint),
            terminate_provisioner_error: Some(RemoteWorkspaceError::NonExistingProvisioner),
            delete_volume_error: Some(RemoteWorkspaceError::NonExistingVolume),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        block_on(service.delete_workspace(&workspace))
            .expect("not-found cleanup errors should be treated as already deleted");

        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
        );
    }

    #[test]
    fn delete_workspace_returns_endpoint_cleanup_failure_and_stops_cleanup() {
        let state = Arc::new(Mutex::new(ProviderState {
            delete_endpoint_error: Some(RemoteWorkspaceError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
            }),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = workspace_with_all_remote_resources(&service);

        let error = block_on(service.delete_workspace(&workspace))
            .expect_err("endpoint cleanup failure should stop cleanup");

        assert_eq!(
            error,
            RemoteWorkspaceError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
            }
        );
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["delete_endpoint"]
        );
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
