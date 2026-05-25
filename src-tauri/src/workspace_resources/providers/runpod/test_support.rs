#![allow(dead_code)]

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
    provider::{
        runpod::{
            RunPodCreateEndpointRequest, RunPodCreateNetworkVolumeRequest, RunPodCreatePodRequest,
            RunPodCreateTemplateRequest, RunPodEndpointObservation, RunPodNetworkVolumeObservation,
            RunPodPodObservation, RunPodTemplateObservation,
        },
        ProviderClientError,
    },
    secrets::{
        HuggingFaceApiKeyStore, ProviderKeyStore, ProvisionerTokenStore,
        ProvisionerWorkerBearerToken, SecretStoreError,
    },
    workspace_catalog::repository::{DeleteWorkspaceResult, WorkspaceCatalogRepository},
    workspace_resources::WorkspaceResourceContext,
    workspace_setup::error::WorkspaceSetupError,
};

use super::client::RunPodWorkspaceResourceClient;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) enum RunPodCall {
    CreateNetworkVolume(RunPodCreateNetworkVolumeRequest),
    GetNetworkVolume(String),
    DiscoverNetworkVolumes(String),
    DeleteNetworkVolume(String),
    CreatePod(RunPodCreatePodRequest),
    GetPod(String),
    DiscoverPods(String),
    DeletePod(String),
    CreateTemplate(RunPodCreateTemplateRequest),
    GetTemplate(String),
    DiscoverTemplates(String),
    DeleteTemplate(String),
    CreateEndpoint(RunPodCreateEndpointRequest),
    GetEndpoint(String),
    DiscoverEndpoints(String),
    DeleteEndpoint(String),
}

#[derive(Debug, Default)]
pub(super) struct FakeRunPodClient {
    calls: Mutex<Vec<RunPodCall>>,
    create_network_volume_results:
        Mutex<VecDeque<Result<RunPodNetworkVolumeObservation, ProviderClientError>>>,
    get_network_volume_results:
        Mutex<VecDeque<Result<RunPodNetworkVolumeObservation, ProviderClientError>>>,
    discover_network_volume_results:
        Mutex<VecDeque<Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>>>,
    delete_network_volume_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
    create_pod_results: Mutex<VecDeque<Result<RunPodPodObservation, ProviderClientError>>>,
    get_pod_results: Mutex<VecDeque<Result<RunPodPodObservation, ProviderClientError>>>,
    discover_pod_results: Mutex<VecDeque<Result<Vec<RunPodPodObservation>, ProviderClientError>>>,
    delete_pod_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
    create_template_results:
        Mutex<VecDeque<Result<RunPodTemplateObservation, ProviderClientError>>>,
    get_template_results: Mutex<VecDeque<Result<RunPodTemplateObservation, ProviderClientError>>>,
    discover_template_results:
        Mutex<VecDeque<Result<Vec<RunPodTemplateObservation>, ProviderClientError>>>,
    delete_template_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
    create_endpoint_results:
        Mutex<VecDeque<Result<RunPodEndpointObservation, ProviderClientError>>>,
    get_endpoint_results: Mutex<VecDeque<Result<RunPodEndpointObservation, ProviderClientError>>>,
    discover_endpoint_results:
        Mutex<VecDeque<Result<Vec<RunPodEndpointObservation>, ProviderClientError>>>,
    delete_endpoint_results: Mutex<VecDeque<Result<(), ProviderClientError>>>,
}

impl FakeRunPodClient {
    pub(super) fn calls(&self) -> Vec<RunPodCall> {
        self.calls.lock().expect("fake runpod calls").clone()
    }

    pub(super) fn push_create_network_volume(
        &self,
        result: Result<RunPodNetworkVolumeObservation, ProviderClientError>,
    ) {
        self.create_network_volume_results
            .lock()
            .expect("fake create volume results")
            .push_back(result);
    }

    pub(super) fn push_get_network_volume(
        &self,
        result: Result<RunPodNetworkVolumeObservation, ProviderClientError>,
    ) {
        self.get_network_volume_results
            .lock()
            .expect("fake get volume results")
            .push_back(result);
    }

    pub(super) fn push_discover_network_volumes(
        &self,
        result: Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>,
    ) {
        self.discover_network_volume_results
            .lock()
            .expect("fake discover volume results")
            .push_back(result);
    }

    pub(super) fn push_delete_network_volume(&self, result: Result<(), ProviderClientError>) {
        self.delete_network_volume_results
            .lock()
            .expect("fake delete volume results")
            .push_back(result);
    }

    pub(super) fn push_create_pod(
        &self,
        result: Result<RunPodPodObservation, ProviderClientError>,
    ) {
        self.create_pod_results
            .lock()
            .expect("fake create pod results")
            .push_back(result);
    }

    pub(super) fn push_get_pod(&self, result: Result<RunPodPodObservation, ProviderClientError>) {
        self.get_pod_results
            .lock()
            .expect("fake get pod results")
            .push_back(result);
    }

    pub(super) fn push_discover_pods(
        &self,
        result: Result<Vec<RunPodPodObservation>, ProviderClientError>,
    ) {
        self.discover_pod_results
            .lock()
            .expect("fake discover pod results")
            .push_back(result);
    }

    pub(super) fn push_delete_pod(&self, result: Result<(), ProviderClientError>) {
        self.delete_pod_results
            .lock()
            .expect("fake delete pod results")
            .push_back(result);
    }

    pub(super) fn push_create_template(
        &self,
        result: Result<RunPodTemplateObservation, ProviderClientError>,
    ) {
        self.create_template_results
            .lock()
            .expect("fake create template results")
            .push_back(result);
    }

    pub(super) fn push_get_template(
        &self,
        result: Result<RunPodTemplateObservation, ProviderClientError>,
    ) {
        self.get_template_results
            .lock()
            .expect("fake get template results")
            .push_back(result);
    }

    pub(super) fn push_discover_templates(
        &self,
        result: Result<Vec<RunPodTemplateObservation>, ProviderClientError>,
    ) {
        self.discover_template_results
            .lock()
            .expect("fake discover template results")
            .push_back(result);
    }

    pub(super) fn push_delete_template(&self, result: Result<(), ProviderClientError>) {
        self.delete_template_results
            .lock()
            .expect("fake delete template results")
            .push_back(result);
    }

    pub(super) fn push_create_endpoint(
        &self,
        result: Result<RunPodEndpointObservation, ProviderClientError>,
    ) {
        self.create_endpoint_results
            .lock()
            .expect("fake create endpoint results")
            .push_back(result);
    }

    pub(super) fn push_get_endpoint(
        &self,
        result: Result<RunPodEndpointObservation, ProviderClientError>,
    ) {
        self.get_endpoint_results
            .lock()
            .expect("fake get endpoint results")
            .push_back(result);
    }

    pub(super) fn push_discover_endpoints(
        &self,
        result: Result<Vec<RunPodEndpointObservation>, ProviderClientError>,
    ) {
        self.discover_endpoint_results
            .lock()
            .expect("fake discover endpoint results")
            .push_back(result);
    }

    pub(super) fn push_delete_endpoint(&self, result: Result<(), ProviderClientError>) {
        self.delete_endpoint_results
            .lock()
            .expect("fake delete endpoint results")
            .push_back(result);
    }

    fn record(&self, call: RunPodCall) {
        self.calls.lock().expect("fake runpod calls").push(call);
    }

    fn next<T>(
        queue: &Mutex<VecDeque<Result<T, ProviderClientError>>>,
        label: &str,
    ) -> Result<T, ProviderClientError> {
        queue
            .lock()
            .expect(label)
            .pop_front()
            .unwrap_or_else(|| panic!("missing fake result for {label}"))
    }
}

impl RunPodWorkspaceResourceClient for FakeRunPodClient {
    fn create_network_volume<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateNetworkVolumeRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        self.record(RunPodCall::CreateNetworkVolume(request.clone()));
        Box::pin(async move {
            Self::next(
                &self.create_network_volume_results,
                "fake create volume results",
            )
        })
    }

    fn get_network_volume<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodNetworkVolumeObservation, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        self.record(RunPodCall::GetNetworkVolume(volume_id.to_string()));
        Box::pin(
            async move { Self::next(&self.get_network_volume_results, "fake get volume results") },
        )
    }

    fn find_network_volumes_by_name<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodNetworkVolumeObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        self.record(RunPodCall::DiscoverNetworkVolumes(name.to_string()));
        Box::pin(async move {
            Self::next(
                &self.discover_network_volume_results,
                "fake discover volume results",
            )
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        self.record(RunPodCall::DeleteNetworkVolume(volume_id.to_string()));
        Box::pin(async move {
            Self::next(
                &self.delete_network_volume_results,
                "fake delete volume results",
            )
        })
    }

    fn create_pod<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        request: &'a RunPodCreatePodRequest,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>
    {
        self.record(RunPodCall::CreatePod(request.clone()));
        Box::pin(async move { Self::next(&self.create_pod_results, "fake create pod results") })
    }

    fn get_pod<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<RunPodPodObservation, ProviderClientError>> + Send + 'a>>
    {
        self.record(RunPodCall::GetPod(pod_id.to_string()));
        Box::pin(async move { Self::next(&self.get_pod_results, "fake get pod results") })
    }

    fn find_pods_by_name<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodPodObservation>, ProviderClientError>> + Send + 'a,
        >,
    > {
        self.record(RunPodCall::DiscoverPods(name.to_string()));
        Box::pin(async move { Self::next(&self.discover_pod_results, "fake discover pod results") })
    }

    fn delete_pod<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        self.record(RunPodCall::DeletePod(pod_id.to_string()));
        Box::pin(async move { Self::next(&self.delete_pod_results, "fake delete pod results") })
    }

    fn create_template<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateTemplateRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodTemplateObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        self.record(RunPodCall::CreateTemplate(request.clone()));
        Box::pin(async move {
            Self::next(
                &self.create_template_results,
                "fake create template results",
            )
        })
    }

    fn find_templates_by_name<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodTemplateObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        self.record(RunPodCall::DiscoverTemplates(name.to_string()));
        Box::pin(async move {
            Self::next(
                &self.discover_template_results,
                "fake discover template results",
            )
        })
    }

    fn delete_template<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        self.record(RunPodCall::DeleteTemplate(template_id.to_string()));
        Box::pin(async move {
            Self::next(
                &self.delete_template_results,
                "fake delete template results",
            )
        })
    }

    fn create_endpoint<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        request: &'a RunPodCreateEndpointRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        self.record(RunPodCall::CreateEndpoint(request.clone()));
        Box::pin(async move {
            Self::next(
                &self.create_endpoint_results,
                "fake create endpoint results",
            )
        })
    }

    fn get_endpoint<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<RunPodEndpointObservation, ProviderClientError>> + Send + 'a,
        >,
    > {
        self.record(RunPodCall::GetEndpoint(endpoint_id.to_string()));
        Box::pin(async move { Self::next(&self.get_endpoint_results, "fake get endpoint results") })
    }

    fn find_endpoints_by_name<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        name: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<RunPodEndpointObservation>, ProviderClientError>>
                + Send
                + 'a,
        >,
    > {
        self.record(RunPodCall::DiscoverEndpoints(name.to_string()));
        Box::pin(async move {
            Self::next(
                &self.discover_endpoint_results,
                "fake discover endpoint results",
            )
        })
    }

    fn delete_endpoint<'a>(
        &'a self,
        _api_key: &'a ProviderApiKey,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderClientError>> + Send + 'a>> {
        self.record(RunPodCall::DeleteEndpoint(endpoint_id.to_string()));
        Box::pin(async move {
            Self::next(
                &self.delete_endpoint_results,
                "fake delete endpoint results",
            )
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct FakeSecretStore {
    state: Arc<Mutex<FakeSecretStoreState>>,
}

#[derive(Debug)]
struct FakeSecretStoreState {
    api_key: Option<ProviderApiKey>,
    hugging_face_api_key: Option<HuggingFaceApiKey>,
    write_tokens: Vec<(String, String)>,
    delete_token_calls: Vec<String>,
    read_hugging_face_key_error: Option<SecretStoreError>,
    write_token_error: Option<SecretStoreError>,
    delete_token_error: Option<SecretStoreError>,
}

impl Default for FakeSecretStore {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeSecretStoreState {
                api_key: Some(
                    ProviderApiKey::new("rp_test_key".to_string())
                        .expect("test api key should be valid"),
                ),
                hugging_face_api_key: None,
                write_tokens: Vec::new(),
                delete_token_calls: Vec::new(),
                read_hugging_face_key_error: None,
                write_token_error: None,
                delete_token_error: None,
            })),
        }
    }
}

impl FakeSecretStore {
    pub(super) fn write_tokens(&self) -> Vec<(String, String)> {
        self.state
            .lock()
            .expect("fake secret store")
            .write_tokens
            .clone()
    }

    pub(super) fn delete_token_calls(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("fake secret store")
            .delete_token_calls
            .clone()
    }

    pub(super) fn fail_delete_token(&self, error: SecretStoreError) {
        self.state
            .lock()
            .expect("fake secret store")
            .delete_token_error = Some(error);
    }

    pub(super) fn set_hugging_face_api_key(&self, value: &str) {
        self.state
            .lock()
            .expect("fake secret store")
            .hugging_face_api_key =
            Some(HuggingFaceApiKey::new(value.to_string()).expect("test hugging face key"));
    }

    pub(super) fn fail_read_hugging_face_api_key(&self, error: SecretStoreError) {
        self.state
            .lock()
            .expect("fake secret store")
            .read_hugging_face_key_error = Some(error);
    }
}

impl ProviderKeyStore for FakeSecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(self
            .state
            .lock()
            .expect("fake secret store")
            .api_key
            .is_some())
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        Ok(self
            .state
            .lock()
            .expect("fake secret store")
            .api_key
            .clone())
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        self.state.lock().expect("fake secret store").api_key = Some(api_key.clone());
        Ok(())
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        self.state.lock().expect("fake secret store").api_key = None;
        Ok(())
    }
}

impl ProvisionerTokenStore for FakeSecretStore {
    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        let mut state = self.state.lock().expect("fake secret store");
        state
            .write_tokens
            .push((workspace_id.to_string(), token.expose_secret().to_string()));
        match state.write_token_error.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn read_provisioner_worker_token(
        &self,
        _workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        Ok(None)
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        let mut state = self.state.lock().expect("fake secret store");
        state.delete_token_calls.push(workspace_id.to_string());
        match state.delete_token_error.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl HuggingFaceApiKeyStore for FakeSecretStore {
    fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError> {
        Ok(self
            .state
            .lock()
            .expect("fake secret store")
            .hugging_face_api_key
            .is_some())
    }

    fn read_hugging_face_api_key(&self) -> Result<Option<HuggingFaceApiKey>, SecretStoreError> {
        let state = self.state.lock().expect("fake secret store");
        match state.read_hugging_face_key_error.clone() {
            Some(error) => Err(error),
            None => Ok(state.hugging_face_api_key.clone()),
        }
    }

    fn replace_hugging_face_api_key(
        &self,
        api_key: &HuggingFaceApiKey,
    ) -> Result<(), SecretStoreError> {
        self.state
            .lock()
            .expect("fake secret store")
            .hugging_face_api_key = Some(api_key.clone());
        Ok(())
    }

    fn delete_hugging_face_api_key(&self) -> Result<(), SecretStoreError> {
        self.state
            .lock()
            .expect("fake secret store")
            .hugging_face_api_key = None;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(super) struct FakeWorkspaceCatalog {
    updates: Mutex<Vec<Workspace>>,
}

impl FakeWorkspaceCatalog {
    pub(super) fn updates(&self) -> Vec<Workspace> {
        self.updates.lock().expect("fake catalog updates").clone()
    }
}

impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        _id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn insert_workspace<'a>(
        &'a self,
        _workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            self.updates
                .lock()
                .expect("fake catalog updates")
                .push(workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn delete_workspace<'a>(
        &'a self,
        _id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<DeleteWorkspaceResult, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }
}

pub(super) fn context<'a>(
    secrets: &'a FakeSecretStore,
    catalog: &'a FakeWorkspaceCatalog,
) -> WorkspaceResourceContext<'a, FakeSecretStore, FakeWorkspaceCatalog> {
    WorkspaceResourceContext::new(secrets, catalog)
}

pub(super) fn workspace() -> Workspace {
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
    let mut workspace = Workspace::new_draft(
        GpuCloudProviderId::Runpod,
        "workspace-1".to_string(),
        "Workspace".to_string(),
        placement_plan,
        runtime,
        provisioner,
    )
    .expect("test workspace should be valid");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace
}

pub(super) fn workspace_requiring_hugging_face_api_key() -> Workspace {
    let mut workspace = workspace();
    let mut preset = workspace.placement_plan.selected_workflow_preset().clone();
    preset.requires_hugging_face_api_key = true;
    preset.required_model_assets.push(ModelAsset {
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
    *selected_workflow_preset = preset;
    workspace
}

pub(super) fn volume_snapshot(status: ProviderResourceStatus) -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: status,
    }
}

pub(super) fn pod_snapshot(status: ProviderResourceStatus) -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "pod-1".to_string(),
        provider_resource_status: status,
        provisioner_status_url: "https://pod/status".to_string(),
    }
}

pub(super) fn endpoint_snapshot(status: ProviderResourceStatus) -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        provider_resource_status: status,
        endpoint_invoke_url: "https://endpoint/run".to_string(),
        provider_metadata: Some(ServerlessEndpointProviderMetadata::Runpod {
            template_id: "template-1".to_string(),
        }),
    }
}

pub(super) fn runpod_volume(
    id: &str,
    status: ProviderResourceStatus,
) -> RunPodNetworkVolumeObservation {
    RunPodNetworkVolumeObservation {
        id: id.to_string(),
        status,
    }
}

pub(super) fn runpod_pod(
    id: &str,
    status: ProviderResourceStatus,
    provisioner_status_url: Option<&str>,
) -> RunPodPodObservation {
    RunPodPodObservation {
        id: id.to_string(),
        status,
        provisioner_status_url: provisioner_status_url.map(str::to_string),
    }
}

pub(super) fn runpod_template(
    id: &str,
    status: ProviderResourceStatus,
    image_name: &str,
) -> RunPodTemplateObservation {
    RunPodTemplateObservation {
        id: id.to_string(),
        image_name: image_name.to_string(),
        status,
    }
}

pub(super) fn runpod_endpoint(
    id: &str,
    status: ProviderResourceStatus,
) -> RunPodEndpointObservation {
    RunPodEndpointObservation {
        id: id.to_string(),
        status,
        endpoint_invoke_url: format!("https://endpoint/{id}/run"),
    }
}
