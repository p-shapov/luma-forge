use std::sync::{Arc, Mutex};

use crate::domain::{
    placement::{
        RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits, RemoteGpuPlacementOption,
        RemotePlacementOptions, RemotePlacementPlan,
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
        ProvisionedRemoteComputeProvisioningStatus, ProvisionedRemoteComputeVolumeSnapshot,
        Workspace, WorkspaceRuntime,
    },
};

use crate::provisioned_remote_compute::{
    errors::ProvisionedRemoteComputeError,
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, ProvisionedRemoteComputeEndpointProvider,
        ProvisionedRemoteComputePlacementOptionsProvider, ProvisionedRemoteComputeProvider,
        ProvisionedRemoteComputeProvisionerProvider, ProvisionedRemoteComputeVolumeProvider,
        StartProvisionerParams, TerminateProvisionerParams,
    },
    registry::ProvisionedRemoteComputeProviderRegistry,
    service::{ProvisionedRemoteComputeService, SetupProvisionedRemoteComputeWorkspaceRequest},
};
use crate::shared::AppFuture;
use crate::workflow_catalog::WorkflowCatalogService;

#[derive(Default)]
pub(crate) struct ProviderState {
    pub(crate) calls: Vec<&'static str>,
    pub(crate) placement_options_result:
        Option<Result<RemotePlacementOptions, ProvisionedRemoteComputeError>>,
    pub(crate) create_volume_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) create_endpoint_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) start_provisioner_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) delete_endpoint_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) terminate_provisioner_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) delete_volume_error: Option<ProvisionedRemoteComputeError>,
    pub(crate) provisioner_status_results:
        Vec<Result<ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeError>>,
    pub(crate) last_create_volume_params: Option<CreateVolumeParams>,
    pub(crate) last_create_endpoint_params: Option<CreateEndpointParams>,
    pub(crate) last_start_provisioner_params: Option<StartProvisionerParams>,
    pub(crate) last_get_provisioner_status_params: Option<GetProvisionerStatusParams>,
}

pub(crate) fn provider_request_failed(message: &str) -> ProvisionedRemoteComputeError {
    ProviderApiError::RequestFailed {
        message: message.to_string(),
    }
    .into()
}

pub(crate) fn placement_options() -> RemotePlacementOptions {
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
    ) -> AppFuture<'a, Result<ProvisionedRemoteComputeVolumeSnapshot, ProvisionedRemoteComputeError>>
    {
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

pub(crate) fn service_with_state(
    state: Arc<Mutex<ProviderState>>,
) -> ProvisionedRemoteComputeService {
    ProvisionedRemoteComputeService::new(
        ProvisionedRemoteComputeProviderRegistry::new(vec![Box::new(FakeProvider::new(state))]),
        WorkflowCatalogService::new(),
    )
}

pub(crate) fn workflow_preset() -> WorkflowPreset {
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

pub(crate) fn placement_plan() -> RemotePlacementPlan {
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

pub(crate) fn draft_workspace(service: &ProvisionedRemoteComputeService) -> Workspace {
    service
        .setup_workspace(SetupProvisionedRemoteComputeWorkspaceRequest {
            workspace_id: "workspace".to_string(),
            workflow_preset: workflow_preset(),
            remote_placement: placement_plan(),
        })
        .expect("workspace setup should succeed")
}

pub(crate) fn workspace_with_all_remote_resources(
    service: &ProvisionedRemoteComputeService,
) -> Workspace {
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

pub(crate) fn failed_cleanup_workspace(workspace: &Workspace, message: &str) -> Workspace {
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

pub(crate) fn block_on<F: std::future::Future>(future: F) -> F::Output {
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
