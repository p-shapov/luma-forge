use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    domain::{
        hugging_face_setup::HuggingFaceApiKey,
        placement::PlacementPlan,
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        provisioner::ResolvedProvisionerImageSnapshot,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{
            ModelAsset, ModelAssetSource, ProvisionerContractReference, RuntimeContractReference,
            WorkflowExecutionType, WorkflowPreset,
        },
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderResourceStatus, ProvisioningPodSnapshot,
            ServerlessEndpointProviderMetadata, ServerlessEndpointSnapshot, Workspace,
            WorkspaceCatalog, WorkspaceLifecycleState,
        },
    },
    secrets::{
        HuggingFaceApiKeyStore, ProviderKeyStore, ProvisionerTokenStore,
        ProvisionerWorkerBearerToken, SecretStoreError,
    },
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{WorkspaceResourceError, WorkspaceResourceOperationResult},
    workspace_setup::error::WorkspaceSetupError,
};

use super::{
    context::{WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    gateway::{
        ProvisionerWorkerError, ProvisionerWorkerGateway, ProvisionerWorkerStartRequest,
        ProvisionerWorkerStatus,
    },
    provisioner::WorkspaceProvisionerService,
    WorkspaceProvisioningConfig, WorkspaceProvisioningCoordinator, WorkspaceProvisioningService,
};

pub(crate) type TestService = WorkspaceProvisioningService<
    FakeSecretStore,
    FakeWorkspaceCatalog,
    FakeProvisionerWorkerGateway,
    FakeWorkspaceResources,
>;

pub(crate) struct TestHarness {
    pub(crate) secrets: FakeSecretStore,
    pub(crate) catalog: FakeWorkspaceCatalog,
    pub(crate) resources: FakeWorkspaceResources,
    pub(crate) workers: FakeProvisionerWorkerGateway,
    workspace_provisioner: WorkspaceProvisionerService,
}

impl TestHarness {
    pub(crate) fn new(workspace: Workspace) -> Self {
        Self::with_secrets(workspace, FakeSecretStore::with_api_key("provider-secret"))
    }

    pub(crate) fn with_secrets(workspace: Workspace, secrets: FakeSecretStore) -> Self {
        Self {
            secrets,
            catalog: FakeWorkspaceCatalog::with_workspace(workspace),
            resources: FakeWorkspaceResources::default(),
            workers: FakeProvisionerWorkerGateway::default(),
            workspace_provisioner: WorkspaceProvisionerService::new(),
        }
    }

    pub(crate) fn context(
        &self,
    ) -> WorkspaceProvisioningContext<
        '_,
        FakeSecretStore,
        FakeWorkspaceCatalog,
        FakeProvisionerWorkerGateway,
        FakeWorkspaceResources,
    > {
        WorkspaceProvisioningContext::new(
            &self.secrets,
            &self.resources,
            &self.catalog,
            &self.workers,
            &self.workspace_provisioner,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FakeSecretStore {
    api_key_result: Arc<Mutex<Result<Option<String>, SecretStoreError>>>,
    worker_token_result: Arc<Mutex<Result<Option<String>, SecretStoreError>>>,
    hugging_face_key_result: Arc<Mutex<Result<Option<String>, SecretStoreError>>>,
    read_api_key_calls: Arc<Mutex<Vec<GpuCloudProviderId>>>,
    read_worker_token_calls: Arc<Mutex<Vec<String>>>,
    has_hugging_face_key_calls: Arc<Mutex<u32>>,
    read_hugging_face_key_calls: Arc<Mutex<u32>>,
}

impl FakeSecretStore {
    pub(crate) fn with_api_key(value: &str) -> Self {
        Self {
            api_key_result: Arc::new(Mutex::new(Ok(Some(value.to_string())))),
            worker_token_result: Arc::new(Mutex::new(Ok(Some("worker-token".to_string())))),
            hugging_face_key_result: Arc::new(Mutex::new(Ok(None))),
            read_api_key_calls: Arc::new(Mutex::new(Vec::new())),
            read_worker_token_calls: Arc::new(Mutex::new(Vec::new())),
            has_hugging_face_key_calls: Arc::new(Mutex::new(0)),
            read_hugging_face_key_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn with_api_key_result(result: Result<Option<String>, SecretStoreError>) -> Self {
        let mut store = Self::with_api_key("provider-secret");
        store.api_key_result = Arc::new(Mutex::new(result));
        store
    }

    pub(crate) fn with_worker_token(mut self, value: &str) -> Self {
        self.worker_token_result = Arc::new(Mutex::new(Ok(Some(value.to_string()))));
        self
    }

    pub(crate) fn with_hugging_face_api_key(mut self, value: &str) -> Self {
        self.hugging_face_key_result = Arc::new(Mutex::new(Ok(Some(value.to_string()))));
        self
    }

    pub(crate) fn read_api_key_calls(&self) -> Vec<GpuCloudProviderId> {
        self.read_api_key_calls
            .lock()
            .expect("fake api key calls")
            .clone()
    }

    pub(crate) fn read_worker_token_calls(&self) -> Vec<String> {
        self.read_worker_token_calls
            .lock()
            .expect("fake worker token calls")
            .clone()
    }

    pub(crate) fn has_hugging_face_key_call_count(&self) -> u32 {
        *self
            .has_hugging_face_key_calls
            .lock()
            .expect("fake hugging face has calls")
    }

    pub(crate) fn read_hugging_face_key_call_count(&self) -> u32 {
        *self
            .read_hugging_face_key_calls
            .lock()
            .expect("fake hugging face read calls")
    }
}

impl ProviderKeyStore for FakeSecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(self
            .api_key_result
            .lock()
            .expect("fake api key result")
            .as_ref()
            .ok()
            .and_then(|value| value.as_ref())
            .is_some())
    }

    fn read_api_key(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        self.read_api_key_calls
            .lock()
            .expect("fake api key calls")
            .push(*provider_id);
        self.api_key_result
            .lock()
            .expect("fake api key result")
            .clone()
            .and_then(|value| {
                value
                    .map(ProviderApiKey::new)
                    .transpose()
                    .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
            })
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        Ok(())
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        Ok(())
    }
}

impl ProvisionerTokenStore for FakeSecretStore {
    fn write_provisioner_worker_token(
        &self,
        _workspace_id: &str,
        _token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        Ok(())
    }

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        self.read_worker_token_calls
            .lock()
            .expect("fake worker token calls")
            .push(workspace_id.to_string());
        self.worker_token_result
            .lock()
            .expect("fake worker token result")
            .clone()
            .and_then(|value| {
                value
                    .map(ProvisionerWorkerBearerToken::new)
                    .transpose()
                    .map_err(|_| SecretStoreError::InvalidStoredProvisionerWorkerToken)
            })
    }

    fn delete_provisioner_worker_token(&self, _workspace_id: &str) -> Result<(), SecretStoreError> {
        Ok(())
    }
}

impl HuggingFaceApiKeyStore for FakeSecretStore {
    fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError> {
        *self
            .has_hugging_face_key_calls
            .lock()
            .expect("fake hugging face has calls") += 1;
        self.hugging_face_key_result
            .lock()
            .expect("fake hugging face key result")
            .clone()
            .map(|value| value.is_some())
    }

    fn read_hugging_face_api_key(&self) -> Result<Option<HuggingFaceApiKey>, SecretStoreError> {
        *self
            .read_hugging_face_key_calls
            .lock()
            .expect("fake hugging face read calls") += 1;
        self.hugging_face_key_result
            .lock()
            .expect("fake hugging face key result")
            .clone()
            .and_then(|value| {
                value
                    .map(HuggingFaceApiKey::new)
                    .transpose()
                    .map_err(|_| SecretStoreError::InvalidStoredHuggingFaceApiKey)
            })
    }

    fn replace_hugging_face_api_key(
        &self,
        _api_key: &HuggingFaceApiKey,
    ) -> Result<(), SecretStoreError> {
        Ok(())
    }

    fn delete_hugging_face_api_key(&self) -> Result<(), SecretStoreError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FakeWorkspaceCatalog {
    workspace: Arc<Mutex<Option<Workspace>>>,
    find_result: SharedFindWorkspaceResult,
    update_result: SharedUpdateWorkspaceResult,
    updates: Arc<Mutex<Vec<Workspace>>>,
}

type FindWorkspaceResult = Result<Option<Workspace>, WorkspaceSetupError>;
type SharedFindWorkspaceResult = Arc<Mutex<Option<FindWorkspaceResult>>>;
type UpdateWorkspaceResult = Result<Workspace, WorkspaceSetupError>;
type SharedUpdateWorkspaceResult = Arc<Mutex<Option<UpdateWorkspaceResult>>>;

impl FakeWorkspaceCatalog {
    pub(crate) fn with_workspace(workspace: Workspace) -> Self {
        Self {
            workspace: Arc::new(Mutex::new(Some(workspace))),
            find_result: Arc::new(Mutex::new(None)),
            update_result: Arc::new(Mutex::new(None)),
            updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self::with_find_error(WorkspaceSetupError::WorkspaceCatalogUnavailable)
    }

    pub(crate) fn with_find_error(error: WorkspaceSetupError) -> Self {
        Self {
            workspace: Arc::new(Mutex::new(None)),
            find_result: Arc::new(Mutex::new(Some(Err(error)))),
            update_result: Arc::new(Mutex::new(None)),
            updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn missing() -> Self {
        Self {
            workspace: Arc::new(Mutex::new(None)),
            find_result: Arc::new(Mutex::new(Some(Ok(None)))),
            update_result: Arc::new(Mutex::new(None)),
            updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn push_update_error(&self, error: WorkspaceSetupError) {
        *self.update_result.lock().expect("fake update result") = Some(Err(error));
    }

    pub(crate) fn updates(&self) -> Vec<Workspace> {
        self.updates.lock().expect("fake updates").clone()
    }

    pub(crate) fn stored_workspace(&self) -> Option<Workspace> {
        self.workspace.lock().expect("fake workspace").clone()
    }
}

impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async {
            Ok(WorkspaceCatalog {
                workspaces: Vec::new(),
            })
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        _id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            if let Some(result) = self.find_result.lock().expect("fake find result").clone() {
                return result;
            }
            Ok(self.workspace.lock().expect("fake workspace").clone())
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            *self.workspace.lock().expect("fake workspace") = Some(workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            self.updates
                .lock()
                .expect("fake updates")
                .push(workspace.clone());
            if let Some(result) = self
                .update_result
                .lock()
                .expect("fake update result")
                .take()
            {
                return result;
            }
            *self.workspace.lock().expect("fake workspace") = Some(workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn delete_workspace<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            let mut workspace = self.workspace.lock().expect("fake workspace");
            if workspace
                .as_ref()
                .is_some_and(|workspace| workspace.id == id)
            {
                *workspace = None;
                return Ok(());
            }
            Err(WorkspaceSetupError::WorkspaceCatalogQueryFailed)
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FakeWorkspaceResources {
    calls: Arc<Mutex<Vec<&'static str>>>,
    network_volume_results: Arc<Mutex<VecDeque<WorkspaceResourceOperationResult>>>,
    provisioning_pod_results: Arc<Mutex<VecDeque<WorkspaceResourceOperationResult>>>,
    finish_pod_results: Arc<Mutex<VecDeque<WorkspaceResourceOperationResult>>>,
    endpoint_results: Arc<Mutex<VecDeque<WorkspaceResourceOperationResult>>>,
    cleanup_results: Arc<Mutex<VecDeque<Result<Workspace, WorkspaceResourceError>>>>,
}

impl FakeWorkspaceResources {
    pub(crate) fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().expect("fake resource calls").clone()
    }

    pub(crate) fn push_network_volume_result(&self, result: WorkspaceResourceOperationResult) {
        self.network_volume_results
            .lock()
            .expect("fake volume results")
            .push_back(result);
    }

    pub(crate) fn push_provisioning_pod_result(&self, result: WorkspaceResourceOperationResult) {
        self.provisioning_pod_results
            .lock()
            .expect("fake pod results")
            .push_back(result);
    }

    pub(crate) fn push_finish_pod_result(&self, result: WorkspaceResourceOperationResult) {
        self.finish_pod_results
            .lock()
            .expect("fake finish results")
            .push_back(result);
    }

    pub(crate) fn push_endpoint_result(&self, result: WorkspaceResourceOperationResult) {
        self.endpoint_results
            .lock()
            .expect("fake endpoint results")
            .push_back(result);
    }

    pub(crate) fn push_cleanup_result(&self, result: Result<Workspace, WorkspaceResourceError>) {
        self.cleanup_results
            .lock()
            .expect("fake cleanup results")
            .push_back(result);
    }

    fn next_sync(
        queue: &Arc<Mutex<VecDeque<WorkspaceResourceOperationResult>>>,
    ) -> WorkspaceResourceOperationResult {
        queue
            .lock()
            .expect("fake sync results")
            .pop_front()
            .unwrap_or(Ok(None))
    }
}

impl WorkspaceProvisioningResources for FakeWorkspaceResources {
    fn create_network_volume<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("create_network_volume");
            Self::next_sync(&self.network_volume_results)
        })
    }

    fn observe_network_volume<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("observe_network_volume");
            Self::next_sync(&self.network_volume_results)
        })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("create_provisioning_pod");
            Self::next_sync(&self.provisioning_pod_results)
        })
    }

    fn observe_provisioning_pod<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("observe_provisioning_pod");
            Self::next_sync(&self.provisioning_pod_results)
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("delete_provisioning_pod");
            Self::next_sync(&self.finish_pod_results)
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("create_endpoint");
            Self::next_sync(&self.endpoint_results)
        })
    }

    fn observe_serverless_endpoint<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("observe_endpoint");
            Self::next_sync(&self.endpoint_results)
        })
    }

    fn cleanup_known_resources<'a>(
        &'a self,
        _workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("fake resource calls")
                .push("cleanup");
            self.cleanup_results
                .lock()
                .expect("fake cleanup results")
                .pop_front()
                .unwrap_or(Err(WorkspaceResourceError::ProviderApiUnavailable))
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FakeProvisionerWorkerGateway {
    status_results: Arc<Mutex<VecDeque<Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>>>,
    status_calls: Arc<Mutex<Vec<String>>>,
    start_calls: Arc<Mutex<Vec<String>>>,
}

impl FakeProvisionerWorkerGateway {
    pub(crate) fn push_status_result(
        &self,
        result: Result<ProvisionerWorkerStatus, ProvisionerWorkerError>,
    ) {
        self.status_results
            .lock()
            .expect("fake worker status results")
            .push_back(result);
    }

    pub(crate) fn status_calls(&self) -> Vec<String> {
        self.status_calls
            .lock()
            .expect("fake worker status calls")
            .clone()
    }

    pub(crate) fn start_calls(&self) -> Vec<String> {
        self.start_calls
            .lock()
            .expect("fake worker start calls")
            .clone()
    }
}

impl ProvisionerWorkerGateway for FakeProvisionerWorkerGateway {
    fn start<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
        _request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.start_calls
                .lock()
                .expect("fake worker start calls")
                .push(provisioner_status_url.to_string());
            Err(ProvisionerWorkerError::Conflict)
        })
    }

    fn status<'a>(
        &'a self,
        provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.status_calls
                .lock()
                .expect("fake worker status calls")
                .push(provisioner_status_url.to_string());
            self.status_results
                .lock()
                .expect("fake worker status results")
                .pop_front()
                .unwrap_or(Err(ProvisionerWorkerError::Unreachable))
        })
    }
}

pub(crate) fn service_parts(
    workspace: Workspace,
) -> (
    TestService,
    FakeSecretStore,
    FakeWorkspaceCatalog,
    FakeWorkspaceResources,
    FakeProvisionerWorkerGateway,
    WorkspaceProvisioningCoordinator,
) {
    let secrets = FakeSecretStore::with_api_key("provider-secret");
    let catalog = FakeWorkspaceCatalog::with_workspace(workspace);
    let resources = FakeWorkspaceResources::default();
    let workers = FakeProvisionerWorkerGateway::default();
    let coordinator = WorkspaceProvisioningCoordinator::default();
    let service = WorkspaceProvisioningService::new(
        secrets.clone(),
        resources.clone(),
        catalog.clone(),
        workers.clone(),
        coordinator.clone(),
        WorkspaceProvisioningConfig,
    );

    (service, secrets, catalog, resources, workers, coordinator)
}

pub(crate) fn workspace() -> Workspace {
    let preset = WorkflowPreset {
        id: "preset-1".to_string(),
        version: "1.0.0".to_string(),
        name: "Preset".to_string(),
        workflow_execution_type: WorkflowExecutionType::T2i,
        required_base_volume_size_bytes: 1,
        requires_hugging_face_api_key: false,
        runtime_contract: RuntimeContractReference {
            id: "runtime".to_string(),
            version: "1.0.0".to_string(),
        },
        provisioner_contract: ProvisionerContractReference {
            id: "provisioner".to_string(),
            version: "1.0.0".to_string(),
        },
        required_model_assets: Vec::new(),
    };
    let placement_plan = PlacementPlan::Runpod {
        selected_datacenter_id: "dc-1".to_string(),
        selected_gpu_id: "gpu-1".to_string(),
        persistent_storage_volume_size_bytes: 1,
        endpoint_keep_alive_seconds: 5,
        selected_workflow_preset: preset,
    };
    let runtime = ResolvedRuntimeImageSnapshot {
        contract_id: "runtime".to_string(),
        contract_version: "1.0.0".to_string(),
        endpoint_image_ref: "endpoint:latest".to_string(),
    };
    let provisioner = ResolvedProvisionerImageSnapshot {
        contract_id: "provisioner".to_string(),
        contract_version: "1.0.0".to_string(),
        provisioner_worker_image_ref: "provisioner:latest".to_string(),
    };
    Workspace::new_draft(
        GpuCloudProviderId::Runpod,
        "workspace-1".to_string(),
        "Workspace".to_string(),
        placement_plan,
        runtime,
        provisioner,
    )
    .expect("test workspace should be valid")
}

pub(crate) fn workspace_requiring_hugging_face_api_key() -> Workspace {
    let mut workspace = workspace();
    let preset = workspace.placement_plan.selected_workflow_preset().clone();
    let mut required_model_assets = preset.required_model_assets;
    required_model_assets.push(ModelAsset {
        id: "asset-1".to_string(),
        name: "Private model".to_string(),
        download_source: ModelAssetSource::Huggingface {
            repository_id: "owner/private-model".to_string(),
            file_path: "model.safetensors".to_string(),
            revision: "main".to_string(),
        },
        install_comfyui_relative_path: "models/checkpoints/model.safetensors".to_string(),
    });
    let PlacementPlan::Runpod {
        selected_workflow_preset,
        ..
    } = &mut workspace.placement_plan;
    selected_workflow_preset.requires_hugging_face_api_key = true;
    selected_workflow_preset.required_model_assets = required_model_assets;
    workspace
}

pub(crate) fn provisioning_workspace() -> Workspace {
    Workspace {
        lifecycle_state: WorkspaceLifecycleState::Provisioning,
        ..workspace()
    }
}

pub(crate) fn provisioning_workspace_requiring_hugging_face_api_key() -> Workspace {
    Workspace {
        lifecycle_state: WorkspaceLifecycleState::Provisioning,
        ..workspace_requiring_hugging_face_api_key()
    }
}

pub(crate) fn volume(status: ProviderResourceStatus) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: status,
    }
}

pub(crate) fn pod(status: ProviderResourceStatus) -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "pod-1".to_string(),
        provider_resource_status: status,
        provisioner_status_url: "https://worker.example/status".to_string(),
    }
}

pub(crate) fn endpoint(status: ProviderResourceStatus) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        provider_resource_status: status,
        endpoint_invoke_url: "https://endpoint.example/run".to_string(),
        provider_metadata: Some(ServerlessEndpointProviderMetadata::Runpod {
            template_id: "template-1".to_string(),
        }),
    }
}

pub(crate) fn ready_provisioning_workspace() -> Workspace {
    let mut workspace = provisioning_workspace();
    workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
    workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Ready));
    workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
    workspace
}
