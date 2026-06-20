use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    domain::{
        runpod::{RunpodPlacementOptions, RunpodPlacementPlan, RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    provider::runpod::{
        errors::RunpodProviderError,
        runtime::{
            CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
            CreateRunpodServerlessTemplateParams, RunpodProvisionerStatus, RunpodRuntimeClient,
            StartRunpodProvisionerPodParams,
        },
    },
    shared::AppFuture,
    workspace::CreateRunpodWorkspaceRequest,
};

pub fn draft_create_request(workspace_id: &str) -> CreateRunpodWorkspaceRequest {
    CreateRunpodWorkspaceRequest {
        workspace_id: workspace_id.to_string(),
        workflow_preset_id: "comfyui-hidream-o1-dev".to_string(),
        placement: RunpodPlacementPlan {
            data_center_id: "dc".to_string(),
            gpu_type_id: "gpu".to_string(),
            volume_size_gb: 100,
        },
    }
}

pub fn workspace_with_runpod(workspace_id: &str, state: WorkspaceState) -> Workspace {
    Workspace {
        id: workspace_id.to_string(),
        workflow: WorkflowReference {
            id: "comfyui-hidream-o1-dev".to_string(),
            version: "1.0.0".to_string(),
        },
        state,
        runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
            placement: RunpodPlacementPlan {
                data_center_id: "dc".to_string(),
                gpu_type_id: "gpu".to_string(),
                volume_size_gb: 100,
            },
            resources: RunpodResources {
                network_volume_id: None,
                provisioner_pod_id: None,
                endpoint_id: None,
                template_id: None,
            },
        }),
    }
}

pub fn workspace_with_runpod_resources(workspace_id: &str) -> Workspace {
    Workspace {
        runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
            placement: RunpodPlacementPlan {
                data_center_id: "dc".to_string(),
                gpu_type_id: "gpu".to_string(),
                volume_size_gb: 100,
            },
            resources: RunpodResources {
                network_volume_id: Some("volume".to_string()),
                provisioner_pod_id: Some("provisioner".to_string()),
                endpoint_id: Some("endpoint".to_string()),
                template_id: Some("template".to_string()),
            },
        }),
        ..workspace_with_runpod(workspace_id, WorkspaceState::Ready)
    }
}

pub fn runpod_client_with_state() -> Arc<dyn RunpodRuntimeClient> {
    Arc::new(FakeRunpodRuntimeClient::default())
}

pub fn runpod_client_with_failure(failure: RunpodClientFailure) -> Arc<dyn RunpodRuntimeClient> {
    Arc::new(FakeRunpodRuntimeClient {
        failure: Some(failure),
        ..Default::default()
    })
}

pub fn runpod_client_with_failed_provisioner() -> Arc<dyn RunpodRuntimeClient> {
    Arc::new(FakeRunpodRuntimeClient {
        provisioner_status: RunpodProvisionerStatus::Failed {
            message: "asset_download_failed: download failed".to_string(),
        },
        ..Default::default()
    })
}

pub fn runpod_client_with_transient_unavailable_provisioner() -> Arc<dyn RunpodRuntimeClient> {
    Arc::new(FakeRunpodRuntimeClient {
        unavailable_status_polls: Arc::new(AtomicUsize::new(1)),
        ..Default::default()
    })
}

pub fn runpod_client_with_provisioner_status_sequence(
    statuses: Vec<Result<RunpodProvisionerStatus, RunpodProviderError>>,
) -> Arc<dyn RunpodRuntimeClient> {
    Arc::new(FakeRunpodRuntimeClient {
        provisioner_status_sequence: Arc::new(Mutex::new(VecDeque::from(statuses))),
        ..Default::default()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodClientFailure {
    CreateNetworkVolume,
    DeleteNetworkVolume,
    StartProvisionerPod,
    TerminateProvisionerPod,
    GetProvisionerStatus,
    CreateServerlessTemplate,
    CreateServerlessEndpoint,
    DeleteEndpoint,
    DeleteTemplate,
}

impl RunpodClientFailure {
    pub fn message(self) -> &'static str {
        match self {
            Self::CreateNetworkVolume => "create network volume failed",
            Self::DeleteNetworkVolume => "delete network volume failed",
            Self::StartProvisionerPod => "start provisioner pod failed",
            Self::TerminateProvisionerPod => "terminate provisioner pod failed",
            Self::GetProvisionerStatus => "get provisioner status failed",
            Self::CreateServerlessTemplate => "create serverless template failed",
            Self::CreateServerlessEndpoint => "create serverless endpoint failed",
            Self::DeleteEndpoint => "delete endpoint failed",
            Self::DeleteTemplate => "delete template failed",
        }
    }
}

#[derive(Debug, Clone)]
struct FakeRunpodRuntimeClient {
    failure: Option<RunpodClientFailure>,
    provisioner_status: RunpodProvisionerStatus,
    provisioner_status_sequence:
        Arc<Mutex<VecDeque<Result<RunpodProvisionerStatus, RunpodProviderError>>>>,
    unavailable_status_polls: Arc<AtomicUsize>,
}

impl Default for FakeRunpodRuntimeClient {
    fn default() -> Self {
        Self {
            failure: None,
            provisioner_status: RunpodProvisionerStatus::Succeeded,
            provisioner_status_sequence: Arc::new(Mutex::new(VecDeque::new())),
            unavailable_status_polls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl FakeRunpodRuntimeClient {
    fn failed(&self, failure: RunpodClientFailure) -> Option<RunpodProviderError> {
        (self.failure == Some(failure)).then(|| provider_failure(failure))
    }
}

impl RunpodRuntimeClient for FakeRunpodRuntimeClient {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodProviderError>> {
        Box::pin(async move {
            Ok(RunpodPlacementOptions {
                max_volume_size_gb: Some(10),
                datacenters: vec![],
            })
        })
    }

    fn create_network_volume<'a>(
        &'a self,
        _params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::CreateNetworkVolume) {
                return Err(error);
            }
            Ok("volume".to_string())
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        _network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::DeleteNetworkVolume) {
                return Err(error);
            }
            Ok(())
        })
    }

    fn start_provisioner_pod<'a>(
        &'a self,
        _params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::StartProvisionerPod) {
                return Err(error);
            }
            Ok("provisioner".to_string())
        })
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        _provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::TerminateProvisionerPod) {
                return Err(error);
            }
            Ok(())
        })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        _workspace_id: &'a str,
        _provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::GetProvisionerStatus) {
                return Err(error);
            }
            if let Some(status) = self
                .provisioner_status_sequence
                .lock()
                .expect("status sequence lock should succeed")
                .pop_front()
            {
                return status;
            }
            if self.unavailable_status_polls.load(Ordering::SeqCst) > 0 {
                self.unavailable_status_polls.fetch_sub(1, Ordering::SeqCst);
                return Err(RunpodProviderError::ProvisionerWorkerUnavailable {
                    message: "provisioner worker is unavailable".to_string(),
                });
            }
            Ok(self.provisioner_status.clone())
        })
    }

    fn create_serverless_template<'a>(
        &'a self,
        _params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::CreateServerlessTemplate) {
                return Err(error);
            }
            Ok("template".to_string())
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        _params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::CreateServerlessEndpoint) {
                return Err(error);
            }
            Ok("endpoint".to_string())
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        _endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::DeleteEndpoint) {
                return Err(error);
            }
            Ok(())
        })
    }

    fn delete_template<'a>(
        &'a self,
        _template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodProviderError>> {
        Box::pin(async move {
            if let Some(error) = self.failed(RunpodClientFailure::DeleteTemplate) {
                return Err(error);
            }
            Ok(())
        })
    }
}

fn provider_failure(failure: RunpodClientFailure) -> RunpodProviderError {
    provider_api_error(failure.message())
}

fn provider_api_error(message: &str) -> RunpodProviderError {
    RunpodProviderError::ProviderApiError(crate::shared::ApiError::RequestFailed {
        message: message.to_string(),
    })
}
