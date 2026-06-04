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
    errors::{
        CreateVolumeError, ObserveEndpointError, ObserveProvisionerError, ObserveVolumeError,
        RemoteWorkspaceProviderRegistryError, StartProvisionerError, WorkspaceObserveError,
        WorkspaceProvisionError, WorkspaceSetupError,
    },
    provider::{
        CreateVolumeParams, ObserveEndpointParams, ObserveProvisionerParams, ObserveVolumeParams,
        StartProvisionerParams,
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
}

impl RemoteWorkspaceService {
    pub fn new(provider_registry: RemoteWorkspaceProviderRegistry) -> Self {
        Self { provider_registry }
    }

    pub fn setup_workspace(
        &self,
        request: SetupWorkspaceRequest,
    ) -> Result<Workspace, WorkspaceSetupError> {
        if request.workspace_id.trim().is_empty() {
            return Err(WorkspaceSetupError::InvalidRequest {
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

    pub async fn observe_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<(), WorkspaceObserveError> {
        let remote = remote_workspace(workspace);
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self
            .provider_registry
            .for_provider(provider_id)
            .map_err(workspace_observe_registry_error)?;

        if provider
            .observe_volume(ObserveVolumeParams {
                workspace_id: workspace.id.clone(),
            })
            .await
            .map_err(workspace_observe_volume_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingVolume);
        }

        if provider
            .observe_provisioner(ObserveProvisionerParams {
                workspace_id: workspace.id.clone(),
            })
            .await
            .map_err(workspace_observe_provisioner_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingProvisioner);
        }

        if provider
            .observe_endpoint(ObserveEndpointParams {
                workspace_id: workspace.id.clone(),
                endpoint_id: remote
                    .remote_resources
                    .remote_endpoint
                    .as_ref()
                    .map(|endpoint| endpoint.id.clone()),
            })
            .await
            .map_err(workspace_observe_endpoint_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingEndpoint);
        }

        Ok(())
    }

    pub async fn provision_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisionError> {
        let remote = remote_workspace(workspace);

        match &remote.remote_provisioning.status {
            RemoteProvisioningStatus::NotStarted => {
                self.observe_workspace(workspace)
                    .await
                    .map_err(workspace_provision_observe_error)?;

                let provider_id = remote.remote_placement.gpu_cloud_provider_id;
                let provider = self
                    .provider_registry
                    .for_provider(provider_id)
                    .map_err(workspace_provision_registry_error)?;

                let remote_volume = provider
                    .create_volume(CreateVolumeParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        size_bytes: remote.remote_placement.remote_volume_size_bytes,
                        mount_path: "/workspace".to_string(),
                    })
                    .await
                    .map_err(workspace_provision_create_volume_error)?;

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
                    WorkspaceProvisionError::InvalidWorkspaceState {
                        message: "remote volume snapshot is required before provisioner start"
                            .to_string(),
                    },
                )?;
                let provider_id = remote.remote_placement.gpu_cloud_provider_id;
                let provider = self
                    .provider_registry
                    .for_provider(provider_id)
                    .map_err(workspace_provision_registry_error)?;

                let remote_provisioner = provider
                    .start_provisioner(StartProvisionerParams {
                        workspace_id: workspace.id.clone(),
                        datacenter_id: remote.remote_placement.datacenter_id.clone(),
                        gpu_id: remote.remote_placement.gpu_id.clone(),
                        volume_id: remote_volume.id.clone(),
                        provisioner_image_ref: "unresolved-provisioner-image".to_string(),
                        mount_path: "/workspace".to_string(),
                    })
                    .await
                    .map_err(workspace_provision_start_provisioner_error)?;

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
                Err(WorkspaceProvisionError::InvalidWorkspaceState {
                    message:
                        "failed workspace must be deleted or reset before provisioning can continue"
                            .to_string(),
                })
            }
            _ => Err(WorkspaceProvisionError::NotImplemented {
                message: "provisioning step is not implemented in this skeleton".to_string(),
            }),
        }
    }
}

fn remote_workspace(workspace: &Workspace) -> &RemoteWorkspace {
    match &workspace.runtime {
        WorkspaceRuntime::Remote(remote) => remote,
    }
}

fn workspace_observe_registry_error(
    error: RemoteWorkspaceProviderRegistryError,
) -> WorkspaceObserveError {
    match error {
        RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id } => {
            WorkspaceObserveError::MissingProvider { provider_id }
        }
    }
}

fn workspace_observe_volume_error(error: ObserveVolumeError) -> WorkspaceObserveError {
    match error {
        ObserveVolumeError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}

fn workspace_observe_provisioner_error(error: ObserveProvisionerError) -> WorkspaceObserveError {
    match error {
        ObserveProvisionerError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}

fn workspace_observe_endpoint_error(error: ObserveEndpointError) -> WorkspaceObserveError {
    match error {
        ObserveEndpointError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}

fn workspace_provision_registry_error(
    error: RemoteWorkspaceProviderRegistryError,
) -> WorkspaceProvisionError {
    match error {
        RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id } => {
            WorkspaceProvisionError::MissingProvider { provider_id }
        }
    }
}

fn workspace_provision_observe_error(error: WorkspaceObserveError) -> WorkspaceProvisionError {
    match error {
        WorkspaceObserveError::MissingProvider { provider_id } => {
            WorkspaceProvisionError::MissingProvider { provider_id }
        }
        WorkspaceObserveError::ExistingVolume => WorkspaceProvisionError::ExistingVolume,
        WorkspaceObserveError::ExistingProvisioner => WorkspaceProvisionError::ExistingProvisioner,
        WorkspaceObserveError::ExistingEndpoint => WorkspaceProvisionError::ExistingEndpoint,
        WorkspaceObserveError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
}

fn workspace_provision_create_volume_error(error: CreateVolumeError) -> WorkspaceProvisionError {
    match error {
        CreateVolumeError::ExistingVolume => WorkspaceProvisionError::ExistingVolume,
        CreateVolumeError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
}

fn workspace_provision_start_provisioner_error(
    error: StartProvisionerError,
) -> WorkspaceProvisionError {
    match error {
        StartProvisionerError::ExistingProvisioner => WorkspaceProvisionError::ExistingProvisioner,
        StartProvisionerError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
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
        errors::{
            CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
            GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError,
            ObserveVolumeError, StartProvisionerError, TerminateProvisionerError,
        },
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, ObserveEndpointParams, ObserveProvisionerParams,
            ObserveVolumeParams, ProviderFuture, RemoteEndpointProvider, RemoteProvisionerProvider,
            RemoteVolumeProvider, RemoteWorkspaceProvider, StartProvisionerParams,
            TerminateProvisionerParams,
        },
    };

    #[derive(Default)]
    struct ProviderState {
        calls: Vec<&'static str>,
        volume: Option<RemoteVolumeSnapshot>,
        provisioner: Option<RemoteProvisionerSnapshot>,
        endpoint: Option<RemoteEndpointSnapshot>,
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
            _params: CreateVolumeParams,
        ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("create_volume");

                Ok(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_volume<'a>(
            &'a self,
            _params: ObserveVolumeParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_volume");
                Ok(state.volume.clone())
            })
        }
    }

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            _params: StartProvisionerParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("start_provisioner");

                Ok(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_provisioner<'a>(
            &'a self,
            _params: ObserveProvisionerParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_provisioner");
                Ok(state.provisioner.clone())
            })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            _params: GetProvisionerStatusParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>>
        {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>> {
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
        ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_endpoint<'a>(
            &'a self,
            _params: ObserveEndpointParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_endpoint");
                Ok(state.endpoint.clone())
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
    fn observe_workspace_returns_existing_volume_conflict() {
        let state = Arc::new(Mutex::new(ProviderState {
            volume: Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            }),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let error = block_on(service.observe_workspace(&workspace))
            .expect_err("existing volume should be a conflict");

        assert_eq!(error, WorkspaceObserveError::ExistingVolume);
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["observe_volume"]
        );
    }

    #[test]
    fn observe_workspace_returns_existing_provisioner_conflict() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner: Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            }),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let error = block_on(service.observe_workspace(&workspace))
            .expect_err("existing provisioner should be a conflict");

        assert_eq!(error, WorkspaceObserveError::ExistingProvisioner);
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["observe_volume", "observe_provisioner"]
        );
    }

    #[test]
    fn observe_workspace_returns_existing_endpoint_conflict() {
        let state = Arc::new(Mutex::new(ProviderState {
            endpoint: Some(RemoteEndpointSnapshot {
                id: "endpoint".to_string(),
                url: "https://endpoint.example".to_string(),
            }),
            ..ProviderState::default()
        }));
        let service = service_with_state(Arc::clone(&state));
        let workspace = draft_workspace(&service);

        let error = block_on(service.observe_workspace(&workspace))
            .expect_err("existing endpoint should be a conflict");

        assert_eq!(error, WorkspaceObserveError::ExistingEndpoint);
        assert_eq!(
            state.lock().expect("state lock should succeed").calls,
            vec!["observe_volume", "observe_provisioner", "observe_endpoint"]
        );
    }

    #[test]
    fn provision_workspace_not_started_runs_preflight_then_creates_volume_only() {
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
            vec![
                "observe_volume",
                "observe_provisioner",
                "observe_endpoint",
                "create_volume"
            ]
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
