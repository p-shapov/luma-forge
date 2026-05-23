pub mod contracts;
pub mod error;
mod providers;

use crate::{
    domain::{
        placement::validator as placement_validator,
        provider_inventory::validator as provider_inventory_validator,
        provider_setup::GpuCloudProviderId,
        provisioner::ProvisionerCatalog,
        runtime::RuntimeCatalog,
        workflow::WorkflowCatalog,
        workspace::{Workspace, WorkspaceCatalog},
    },
    secrets::AsyncProviderKeyStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use contracts::{CreateWorkspaceInput, ProviderPlacementOptions};
use error::WorkspaceSetupError;
pub use providers::{
    WorkspaceSetupProviderCapability, WorkspaceSetupProviderRegistry,
    WorkspaceSetupProviderResolver,
};

pub trait WorkspaceSetupCatalogReader: Send + Sync {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError>;
    fn runtime_catalog(&self) -> Result<RuntimeCatalog, WorkspaceSetupError>;
    fn provisioner_catalog(&self) -> Result<ProvisionerCatalog, WorkspaceSetupError>;
}

pub struct WorkspaceSetupService<C, S, W, R = WorkspaceSetupProviderRegistry> {
    catalogs: C,
    secrets: S,
    workspace_catalog: W,
    provider_registry: R,
}

impl<C, S, W, R> WorkspaceSetupService<C, S, W, R> {
    pub fn with_provider_registry(
        catalogs: C,
        secrets: S,
        workspace_catalog: W,
        provider_registry: R,
    ) -> Self {
        Self {
            catalogs,
            secrets,
            workspace_catalog,
            provider_registry,
        }
    }
}

impl<C, S, W, R> WorkspaceSetupService<C, S, W, R>
where
    C: WorkspaceSetupCatalogReader,
    S: AsyncProviderKeyStore,
    W: WorkspaceCatalogRepository,
    R: WorkspaceSetupProviderResolver,
{
    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        self.catalogs.workflow_catalog()
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
        let api_key = self
            .secrets
            .read_api_key(&provider_id)
            .await?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;
        let options = self
            .provider_registry
            .for_provider(&provider_id)
            .get_provider_placement_options(&api_key)
            .await?;
        self.validate_provider_placement_options(provider_id, options)
    }

    fn validate_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
        options: ProviderPlacementOptions,
    ) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
        provider_inventory_validator::validate_provider_inventory(
            provider_id,
            &options.provider_inventory,
        )
        .map_err(|_| WorkspaceSetupError::ProviderInventoryInvalid)?;
        Ok(options)
    }

    pub async fn get_workspace_catalog(&self) -> Result<WorkspaceCatalog, WorkspaceSetupError> {
        self.workspace_catalog.list_workspaces().await
    }

    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceInput,
    ) -> Result<Workspace, WorkspaceSetupError> {
        let workspace_id = uuid::Uuid::parse_str(&request.workspace_id)
            .map_err(|_| WorkspaceSetupError::InvalidWorkspaceId)?;
        let name = request.name.trim();
        if name.is_empty() {
            return Err(WorkspaceSetupError::WorkspaceNameRequired);
        }

        let provider_id = request.gpu_cloud_provider_id;
        self.secrets
            .read_api_key(&provider_id)
            .await?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

        let workflow_catalog = self.catalogs.workflow_catalog()?;
        let runtime_catalog = self.catalogs.runtime_catalog()?;
        let provisioner_catalog = self.catalogs.provisioner_catalog()?;
        placement_validator::validate_placement_plan(
            provider_id,
            &request.placement_plan,
            &workflow_catalog,
            &runtime_catalog,
            &provisioner_catalog,
        )
        .map_err(WorkspaceSetupError::from)?;
        let selected_preset = request.placement_plan.selected_workflow_preset();
        let resolved_runtime_image = runtime_catalog
            .resolve(
                &selected_preset.runtime_contract.id,
                &selected_preset.runtime_contract.version,
            )
            .ok_or(WorkspaceSetupError::WorkflowCatalogUnavailable)?;
        let resolved_provisioner_image = provisioner_catalog
            .resolve(
                &selected_preset.provisioner_contract.id,
                &selected_preset.provisioner_contract.version,
            )
            .ok_or(WorkspaceSetupError::WorkflowCatalogUnavailable)?;

        let workspace = Workspace::new_draft(
            provider_id,
            workspace_id.to_string(),
            name.to_string(),
            request.placement_plan,
            resolved_runtime_image,
            resolved_provisioner_image,
        )
        .map_err(|_| WorkspaceSetupError::InvalidWorkspaceMetadata)?;

        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        Ok(workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            placement::{
                EndpointKeepAliveCapability, PlacementPlan, ProviderPlacementCapabilities,
                RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS, RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS,
            },
            provider_inventory::{Datacenter, GpuOption, ProviderInventory},
            provider_setup::ProviderApiKey,
            provisioner::{ProvisionerCatalog, ProvisionerContract, ProvisionerContractRevision},
            runtime::{RuntimeContract, RuntimeContractRevision},
            workflow::{
                ProvisionerContractReference, RuntimeContractReference, WorkflowExecutionType,
                WorkflowPreset,
            },
            workspace::WorkspaceLifecycleState,
        },
        secrets::{ProviderKeyStore, SecretStoreError},
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const REQUIRED_VOLUME_SIZE: u64 = 80 * 1024 * 1024 * 1024;
    const WORKSPACE_ID: &str = "018f47a2-3b19-77aa-8f2c-9271f1eb1234";

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum CatalogCall {
        Workflow,
        Runtime,
        Provisioner,
    }

    #[derive(Debug)]
    struct FakeCatalogReaderState {
        workflow_catalog: Result<WorkflowCatalog, WorkspaceSetupError>,
        runtime_catalog: Result<RuntimeCatalog, WorkspaceSetupError>,
        provisioner_catalog: Result<ProvisionerCatalog, WorkspaceSetupError>,
        calls: Vec<CatalogCall>,
    }

    #[derive(Debug, Clone)]
    struct FakeCatalogReader {
        state: Arc<Mutex<FakeCatalogReaderState>>,
    }

    impl FakeCatalogReader {
        fn valid() -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeCatalogReaderState {
                    workflow_catalog: Ok(workflow_catalog()),
                    runtime_catalog: Ok(runtime_catalog()),
                    provisioner_catalog: Ok(provisioner_catalog()),
                    calls: Vec::new(),
                })),
            }
        }

        fn with_workflow_catalog(self, workflow_catalog: WorkflowCatalog) -> Self {
            self.state
                .lock()
                .expect("fake catalog mutex")
                .workflow_catalog = Ok(workflow_catalog);
            self
        }

        fn with_workflow_error(self, error: WorkspaceSetupError) -> Self {
            self.state
                .lock()
                .expect("fake catalog mutex")
                .workflow_catalog = Err(error);
            self
        }

        fn with_runtime_error(self, error: WorkspaceSetupError) -> Self {
            self.state
                .lock()
                .expect("fake catalog mutex")
                .runtime_catalog = Err(error);
            self
        }

        fn with_provisioner_error(self, error: WorkspaceSetupError) -> Self {
            self.state
                .lock()
                .expect("fake catalog mutex")
                .provisioner_catalog = Err(error);
            self
        }

        fn calls(&self) -> Vec<CatalogCall> {
            self.state.lock().expect("fake catalog mutex").calls.clone()
        }
    }

    impl WorkspaceSetupCatalogReader for FakeCatalogReader {
        fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
            let mut state = self.state.lock().expect("fake catalog mutex");
            state.calls.push(CatalogCall::Workflow);
            state.workflow_catalog.clone()
        }

        fn runtime_catalog(&self) -> Result<RuntimeCatalog, WorkspaceSetupError> {
            let mut state = self.state.lock().expect("fake catalog mutex");
            state.calls.push(CatalogCall::Runtime);
            state.runtime_catalog.clone()
        }

        fn provisioner_catalog(&self) -> Result<ProvisionerCatalog, WorkspaceSetupError> {
            let mut state = self.state.lock().expect("fake catalog mutex");
            state.calls.push(CatalogCall::Provisioner);
            state.provisioner_catalog.clone()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SecretStoreCall {
        HasApiKeyEntry,
        ReadApiKey,
        ReplaceApiKey,
        DeleteApiKey,
    }

    #[derive(Debug)]
    struct FakeSecretStoreState {
        api_key_result: Result<Option<String>, SecretStoreError>,
        calls: Vec<SecretStoreCall>,
    }

    #[derive(Debug, Clone)]
    struct FakeSecretStore {
        state: Arc<Mutex<FakeSecretStoreState>>,
    }

    impl FakeSecretStore {
        fn with_api_key(api_key: impl Into<String>) -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeSecretStoreState {
                    api_key_result: Ok(Some(api_key.into())),
                    calls: Vec::new(),
                })),
            }
        }

        fn missing_api_key() -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeSecretStoreState {
                    api_key_result: Ok(None),
                    calls: Vec::new(),
                })),
            }
        }

        fn with_api_key_result(result: Result<Option<String>, SecretStoreError>) -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeSecretStoreState {
                    api_key_result: result,
                    calls: Vec::new(),
                })),
            }
        }

        fn calls(&self) -> Vec<SecretStoreCall> {
            self.state.lock().expect("fake secret mutex").calls.clone()
        }
    }

    impl ProviderKeyStore for FakeSecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            let mut state = self.state.lock().expect("fake secret mutex");
            state.calls.push(SecretStoreCall::HasApiKeyEntry);
            Ok(matches!(state.api_key_result, Ok(Some(_))))
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            let mut state = self.state.lock().expect("fake secret mutex");
            state.calls.push(SecretStoreCall::ReadApiKey);
            state.api_key_result.clone().and_then(|value| {
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
            self.state
                .lock()
                .expect("fake secret mutex")
                .calls
                .push(SecretStoreCall::ReplaceApiKey);
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            self.state
                .lock()
                .expect("fake secret mutex")
                .calls
                .push(SecretStoreCall::DeleteApiKey);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FakeWorkspaceCatalogState {
        workspaces: Vec<Workspace>,
        inserts: Vec<Workspace>,
        updates: Vec<Workspace>,
        list_error: Option<WorkspaceSetupError>,
        insert_error: Option<WorkspaceSetupError>,
    }

    #[derive(Debug, Clone)]
    struct FakeWorkspaceCatalog {
        state: Arc<Mutex<FakeWorkspaceCatalogState>>,
    }

    impl FakeWorkspaceCatalog {
        fn empty() -> Self {
            Self::with_workspaces([])
        }

        fn with_workspaces(workspaces: impl IntoIterator<Item = Workspace>) -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeWorkspaceCatalogState {
                    workspaces: workspaces.into_iter().collect(),
                    inserts: Vec::new(),
                    updates: Vec::new(),
                    list_error: None,
                    insert_error: None,
                })),
            }
        }

        fn with_insert_error(self, error: WorkspaceSetupError) -> Self {
            self.state
                .lock()
                .expect("fake workspace catalog mutex")
                .insert_error = Some(error);
            self
        }

        fn with_list_error(self, error: WorkspaceSetupError) -> Self {
            self.state
                .lock()
                .expect("fake workspace catalog mutex")
                .list_error = Some(error);
            self
        }

        fn inserts(&self) -> Vec<Workspace> {
            self.state
                .lock()
                .expect("fake workspace catalog mutex")
                .inserts
                .clone()
        }

        fn workspaces(&self) -> Vec<Workspace> {
            self.state
                .lock()
                .expect("fake workspace catalog mutex")
                .workspaces
                .clone()
        }
    }

    impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
        fn list_workspaces<'a>(
            &'a self,
        ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                let state = self.state.lock().expect("fake workspace catalog mutex");
                if let Some(error) = state.list_error.clone() {
                    return Err(error);
                }

                Ok(WorkspaceCatalog {
                    workspaces: state.workspaces.clone(),
                })
            })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                Ok(self
                    .state
                    .lock()
                    .expect("fake workspace catalog mutex")
                    .workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .cloned())
            })
        }

        fn insert_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().expect("fake workspace catalog mutex");
                if let Some(error) = state.insert_error.clone() {
                    return Err(error);
                }
                if state
                    .workspaces
                    .iter()
                    .any(|existing| existing.id == workspace.id)
                {
                    return Err(WorkspaceSetupError::WorkspaceAlreadyExists);
                }

                let workspace = workspace.clone();
                state.inserts.push(workspace.clone());
                state.workspaces.push(workspace.clone());
                Ok(workspace)
            })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().expect("fake workspace catalog mutex");
                state.updates.push(workspace.clone());
                Ok(workspace.clone())
            })
        }
    }

    #[derive(Debug, Default)]
    struct FakePlacementOptionsProvider {
        result: Mutex<Option<Result<ProviderPlacementOptions, WorkspaceSetupError>>>,
        calls: Mutex<Vec<GpuCloudProviderId>>,
    }

    impl FakePlacementOptionsProvider {
        fn with_result(result: Result<ProviderPlacementOptions, WorkspaceSetupError>) -> Arc<Self> {
            Arc::new(Self {
                result: Mutex::new(Some(result)),
                calls: Mutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> Vec<GpuCloudProviderId> {
            self.calls
                .lock()
                .expect("fake placement provider mutex")
                .clone()
        }
    }

    impl WorkspaceSetupProviderResolver for Arc<FakePlacementOptionsProvider> {
        fn for_provider(
            &self,
            provider_id: &GpuCloudProviderId,
        ) -> &dyn WorkspaceSetupProviderCapability {
            self.calls
                .lock()
                .expect("fake placement provider mutex")
                .push(*provider_id);
            self
        }
    }

    impl WorkspaceSetupProviderCapability for Arc<FakePlacementOptionsProvider> {
        fn get_provider_placement_options<'a>(
            &'a self,
            _api_key: &'a ProviderApiKey,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>>
                    + Send
                    + 'a,
            >,
        > {
            let result = self
                .result
                .lock()
                .expect("fake placement provider mutex")
                .clone()
                .expect("fake placement result");

            Box::pin(async move { result })
        }
    }

    fn runtime_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: "1.0.0".to_string(),
                    endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
                }],
            }],
        }
    }

    fn provisioner_catalog() -> ProvisionerCatalog {
        ProvisionerCatalog {
            contracts: vec![ProvisionerContract {
                id: "luma-forge-provisioner".to_string(),
                revisions: vec![ProvisionerContractRevision {
                    version: "1.0.0".to_string(),
                    provisioner_worker_image_ref: format!(
                        "ghcr.io/luma-forge/provisioner@sha256:{DIGEST_B}"
                    ),
                    volume_mount_path: "/workspace".to_string(),
                }],
            }],
        }
    }

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "comfyui-hidream-o1-dev".to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI Text to Image".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: REQUIRED_VOLUME_SIZE,
            runtime_contract: RuntimeContractReference {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: ProvisionerContractReference {
                id: "luma-forge-provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![],
        }
    }

    fn workflow_catalog() -> WorkflowCatalog {
        WorkflowCatalog {
            workflow_presets: vec![workflow_preset()],
        }
    }

    fn placement_plan() -> PlacementPlan {
        placement_plan_with(
            "EU-RO-1",
            "NVIDIA A40",
            REQUIRED_VOLUME_SIZE,
            workflow_preset(),
        )
    }

    fn placement_plan_with(
        datacenter_id: &str,
        gpu_id: &str,
        volume_size: u64,
        selected_workflow_preset: WorkflowPreset,
    ) -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: datacenter_id.to_string(),
            selected_gpu_id: gpu_id.to_string(),
            persistent_storage_volume_size_bytes: volume_size,
            endpoint_keep_alive_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            selected_workflow_preset,
        }
    }

    fn placement_plan_with_keep_alive(keep_alive_seconds: u32) -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA A40".to_string(),
            persistent_storage_volume_size_bytes: REQUIRED_VOLUME_SIZE,
            endpoint_keep_alive_seconds: keep_alive_seconds,
            selected_workflow_preset: workflow_preset(),
        }
    }

    fn create_workspace_input() -> CreateWorkspaceInput {
        CreateWorkspaceInput {
            workspace_id: WORKSPACE_ID.to_string(),
            name: "Text to Image".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan: placement_plan(),
        }
    }

    fn workspace_with_id(id: &str) -> Workspace {
        Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            id.to_string(),
            format!("Workspace {id}"),
            placement_plan(),
            runtime_catalog()
                .resolve("comfyui-hidream-o1-dev-python312-cu121", "1.0.0")
                .expect("runtime should resolve"),
            provisioner_catalog()
                .resolve("luma-forge-provisioner", "1.0.0")
                .expect("provisioner should resolve"),
        )
        .expect("test workspace should be valid")
    }

    fn valid_inventory() -> ProviderInventory {
        ProviderInventory {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            fetched_at: "2026-05-18T00:00:00Z".to_string(),
            max_persistent_storage_volume_size_bytes: Some(100 * 1024 * 1024 * 1024),
            datacenters: vec![Datacenter {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                id: "EU-RO-1".to_string(),
                name: "Europe Romania 1".to_string(),
                gpu_options: vec![GpuOption {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    id: "NVIDIA A40".to_string(),
                    name: "NVIDIA A40".to_string(),
                    vram_bytes: 48 * 1024 * 1024 * 1024,
                    availability_score: 80,
                }],
            }],
        }
    }

    fn placement_options_with_inventory(
        provider_inventory: ProviderInventory,
    ) -> ProviderPlacementOptions {
        ProviderPlacementOptions {
            provider_inventory,
            placement_capabilities: ProviderPlacementCapabilities {
                endpoint_keep_alive: EndpointKeepAliveCapability::Supported {
                    default_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
                    min_seconds: crate::domain::placement::RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS,
                    max_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS,
                },
            },
        }
    }

    fn service(
        catalogs: FakeCatalogReader,
        secrets: FakeSecretStore,
        workspace_catalog: FakeWorkspaceCatalog,
    ) -> WorkspaceSetupService<
        FakeCatalogReader,
        FakeSecretStore,
        FakeWorkspaceCatalog,
        Arc<FakePlacementOptionsProvider>,
    > {
        WorkspaceSetupService::with_provider_registry(
            catalogs,
            secrets,
            workspace_catalog,
            FakePlacementOptionsProvider::with_result(Ok(placement_options_with_inventory(
                valid_inventory(),
            ))),
        )
    }

    fn service_with_provider(
        catalogs: FakeCatalogReader,
        secrets: FakeSecretStore,
        workspace_catalog: FakeWorkspaceCatalog,
        provider: Arc<FakePlacementOptionsProvider>,
    ) -> WorkspaceSetupService<
        FakeCatalogReader,
        FakeSecretStore,
        FakeWorkspaceCatalog,
        Arc<FakePlacementOptionsProvider>,
    > {
        WorkspaceSetupService::with_provider_registry(
            catalogs,
            secrets,
            workspace_catalog,
            provider,
        )
    }

    #[tokio::test]
    async fn create_workspace_persists_complete_draft_workspace() {
        let catalogs = FakeCatalogReader::valid();
        let secrets = FakeSecretStore::with_api_key("rp_key_secret");
        let unrelated = workspace_with_id("018f47a2-3b19-77aa-8f2c-9271f1eb0001");
        let workspace_catalog = FakeWorkspaceCatalog::with_workspaces([unrelated.clone()]);

        let workspace = service(catalogs.clone(), secrets.clone(), workspace_catalog.clone())
            .create_workspace(create_workspace_input())
            .await
            .expect("workspace creation should succeed");

        assert_eq!(workspace.id, WORKSPACE_ID);
        assert_eq!(workspace.name, "Text to Image");
        assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Draft);
        assert_eq!(workspace.gpu_cloud_provider_id, GpuCloudProviderId::Runpod);
        assert_eq!(workspace.placement_plan, placement_plan());
        assert_eq!(workspace.persistent_storage_volume_snapshot, None);
        assert_eq!(workspace.active_provisioning_pod_snapshot, None);
        assert_eq!(workspace.serverless_endpoint_snapshot, None);
        assert_eq!(workspace.last_provisioning_pod_snapshot, None);
        assert_eq!(
            workspace.resolved_runtime_image,
            runtime_catalog()
                .resolve("comfyui-hidream-o1-dev-python312-cu121", "1.0.0")
                .expect("runtime should resolve")
        );
        assert_eq!(
            workspace.resolved_provisioner_image,
            provisioner_catalog()
                .resolve("luma-forge-provisioner", "1.0.0")
                .expect("provisioner should resolve")
        );
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
        assert_eq!(
            catalogs.calls(),
            vec![
                CatalogCall::Workflow,
                CatalogCall::Runtime,
                CatalogCall::Provisioner
            ]
        );
        assert_eq!(workspace_catalog.inserts(), vec![workspace.clone()]);
        assert_eq!(
            workspace_catalog.workspaces(),
            vec![unrelated, workspace],
            "unrelated catalog entries should be preserved"
        );
    }

    #[tokio::test]
    async fn create_workspace_rejects_invalid_uuid_before_other_dependencies() {
        let catalogs = FakeCatalogReader::valid();
        let secrets = FakeSecretStore::with_api_key("rp_key_secret");
        let workspace_catalog = FakeWorkspaceCatalog::empty();
        let mut request = create_workspace_input();
        request.workspace_id = "not-a-uuid".to_string();

        let result = service(catalogs.clone(), secrets.clone(), workspace_catalog.clone())
            .create_workspace(request)
            .await;

        assert_eq!(result, Err(WorkspaceSetupError::InvalidWorkspaceId));
        assert!(catalogs.calls().is_empty());
        assert!(secrets.calls().is_empty());
        assert!(workspace_catalog.inserts().is_empty());
    }

    #[tokio::test]
    async fn create_workspace_rejects_blank_name_before_secret_or_persistence() {
        let catalogs = FakeCatalogReader::valid();
        let secrets = FakeSecretStore::with_api_key("rp_key_secret");
        let workspace_catalog = FakeWorkspaceCatalog::empty();
        let mut request = create_workspace_input();
        request.name = " \n\t".to_string();

        let result = service(catalogs.clone(), secrets.clone(), workspace_catalog.clone())
            .create_workspace(request)
            .await;

        assert_eq!(result, Err(WorkspaceSetupError::WorkspaceNameRequired));
        assert!(catalogs.calls().is_empty());
        assert!(secrets.calls().is_empty());
        assert!(workspace_catalog.inserts().is_empty());
    }

    #[tokio::test]
    async fn create_workspace_rejects_missing_provider_key_before_catalog_or_persistence() {
        let catalogs = FakeCatalogReader::valid();
        let secrets = FakeSecretStore::missing_api_key();
        let workspace_catalog = FakeWorkspaceCatalog::empty();

        let result = service(catalogs.clone(), secrets.clone(), workspace_catalog.clone())
            .create_workspace(create_workspace_input())
            .await;

        assert_eq!(result, Err(WorkspaceSetupError::ProviderSetupIncomplete));
        assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
        assert!(catalogs.calls().is_empty());
        assert!(workspace_catalog.inserts().is_empty());
    }

    #[tokio::test]
    async fn create_workspace_maps_secret_store_errors_before_persistence() {
        for (secret_result, expected_error) in [
            (
                Ok(Some(" \t".to_string())),
                WorkspaceSetupError::StoredProviderApiKeyInvalid,
            ),
            (
                Err(SecretStoreError::SecureKeyringUnavailable),
                WorkspaceSetupError::SecureKeyringUnavailable,
            ),
        ] {
            let catalogs = FakeCatalogReader::valid();
            let secrets = FakeSecretStore::with_api_key_result(secret_result);
            let workspace_catalog = FakeWorkspaceCatalog::empty();

            let result = service(catalogs.clone(), secrets.clone(), workspace_catalog.clone())
                .create_workspace(create_workspace_input())
                .await;

            assert_eq!(result, Err(expected_error));
            assert_eq!(secrets.calls(), vec![SecretStoreCall::ReadApiKey]);
            assert!(catalogs.calls().is_empty());
            assert!(workspace_catalog.inserts().is_empty());
        }
    }

    #[tokio::test]
    async fn create_workspace_propagates_catalog_errors_before_persistence() {
        for (catalogs, expected_error, expected_calls) in [
            (
                FakeCatalogReader::valid()
                    .with_workflow_error(WorkspaceSetupError::WorkflowCatalogUnavailable),
                WorkspaceSetupError::WorkflowCatalogUnavailable,
                vec![CatalogCall::Workflow],
            ),
            (
                FakeCatalogReader::valid()
                    .with_runtime_error(WorkspaceSetupError::WorkflowCatalogUnavailable),
                WorkspaceSetupError::WorkflowCatalogUnavailable,
                vec![CatalogCall::Workflow, CatalogCall::Runtime],
            ),
            (
                FakeCatalogReader::valid()
                    .with_provisioner_error(WorkspaceSetupError::WorkflowCatalogUnavailable),
                WorkspaceSetupError::WorkflowCatalogUnavailable,
                vec![
                    CatalogCall::Workflow,
                    CatalogCall::Runtime,
                    CatalogCall::Provisioner,
                ],
            ),
        ] {
            let secrets = FakeSecretStore::with_api_key("rp_key_secret");
            let workspace_catalog = FakeWorkspaceCatalog::empty();

            let result = service(catalogs.clone(), secrets, workspace_catalog.clone())
                .create_workspace(create_workspace_input())
                .await;

            assert_eq!(result, Err(expected_error));
            assert_eq!(catalogs.calls(), expected_calls);
            assert!(workspace_catalog.inserts().is_empty());
        }
    }

    #[tokio::test]
    async fn create_workspace_maps_placement_validation_errors_before_persistence() {
        let changed_preset = WorkflowPreset {
            name: "Changed".to_string(),
            ..workflow_preset()
        };
        let invalid_cases = [
            (
                placement_plan_with(
                    "EU-RO-1",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE,
                    changed_preset,
                ),
                WorkspaceSetupError::WorkflowPresetStale,
            ),
            (
                placement_plan_with(" ", "NVIDIA A40", REQUIRED_VOLUME_SIZE, workflow_preset()),
                WorkspaceSetupError::PlacementDatacenterRequired,
            ),
            (
                placement_plan_with("EU-RO-1", " ", REQUIRED_VOLUME_SIZE, workflow_preset()),
                WorkspaceSetupError::PlacementGpuRequired,
            ),
            (
                placement_plan_with(
                    "EU-RO-1",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE - 1,
                    workflow_preset(),
                ),
                WorkspaceSetupError::StorageSizeBelowPresetMinimum,
            ),
            (
                placement_plan_with_keep_alive(RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS + 1),
                WorkspaceSetupError::EndpointKeepAliveOutOfRange,
            ),
        ];

        for (placement_plan, expected_error) in invalid_cases {
            let workspace_catalog = FakeWorkspaceCatalog::empty();
            let mut request = create_workspace_input();
            request.placement_plan = placement_plan;

            let result = service(
                FakeCatalogReader::valid(),
                FakeSecretStore::with_api_key("rp_key_secret"),
                workspace_catalog.clone(),
            )
            .create_workspace(request)
            .await;

            assert_eq!(result, Err(expected_error));
            assert!(workspace_catalog.inserts().is_empty());
        }
    }

    #[tokio::test]
    async fn create_workspace_rejects_stale_runtime_contract_before_persistence() {
        let stale_preset = WorkflowPreset {
            runtime_contract: RuntimeContractReference {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                version: "2.0.0".to_string(),
            },
            ..workflow_preset()
        };
        let catalogs = FakeCatalogReader::valid().with_workflow_catalog(WorkflowCatalog {
            workflow_presets: vec![stale_preset.clone()],
        });
        let workspace_catalog = FakeWorkspaceCatalog::empty();
        let mut request = create_workspace_input();
        request.placement_plan =
            placement_plan_with("EU-RO-1", "NVIDIA A40", REQUIRED_VOLUME_SIZE, stale_preset);

        let result = service(
            catalogs,
            FakeSecretStore::with_api_key("rp_key_secret"),
            workspace_catalog.clone(),
        )
        .create_workspace(request)
        .await;

        assert_eq!(result, Err(WorkspaceSetupError::WorkflowPresetStale));
        assert!(workspace_catalog.inserts().is_empty());
    }

    #[tokio::test]
    async fn create_workspace_propagates_persistence_errors() {
        let duplicate_catalog =
            FakeWorkspaceCatalog::with_workspaces([workspace_with_id(WORKSPACE_ID)]);
        let duplicate_result = service(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            duplicate_catalog.clone(),
        )
        .create_workspace(create_workspace_input())
        .await;

        assert_eq!(
            duplicate_result,
            Err(WorkspaceSetupError::WorkspaceAlreadyExists)
        );
        assert!(duplicate_catalog.inserts().is_empty());

        let failing_catalog = FakeWorkspaceCatalog::empty()
            .with_insert_error(WorkspaceSetupError::WorkspaceCatalogQueryFailed);
        let failing_result = service(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            failing_catalog.clone(),
        )
        .create_workspace(create_workspace_input())
        .await;

        assert_eq!(
            failing_result,
            Err(WorkspaceSetupError::WorkspaceCatalogQueryFailed)
        );
        assert!(failing_catalog.inserts().is_empty());
    }

    #[test]
    fn get_workflow_catalog_returns_catalog_or_error() {
        let ok_catalogs = FakeCatalogReader::valid();
        let ok_result = service(
            ok_catalogs.clone(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            FakeWorkspaceCatalog::empty(),
        )
        .get_workflow_catalog();

        assert_eq!(ok_result, Ok(workflow_catalog()));
        assert_eq!(ok_catalogs.calls(), vec![CatalogCall::Workflow]);

        let error_catalogs = FakeCatalogReader::valid()
            .with_workflow_error(WorkspaceSetupError::WorkflowCatalogUnavailable);
        let error_result = service(
            error_catalogs,
            FakeSecretStore::with_api_key("rp_key_secret"),
            FakeWorkspaceCatalog::empty(),
        )
        .get_workflow_catalog();

        assert_eq!(
            error_result,
            Err(WorkspaceSetupError::WorkflowCatalogUnavailable)
        );
    }

    #[tokio::test]
    async fn get_workspace_catalog_delegates_to_repository() {
        let existing = workspace_with_id("018f47a2-3b19-77aa-8f2c-9271f1eb0002");
        let workspace_catalog = FakeWorkspaceCatalog::with_workspaces([existing.clone()]);
        let result = service(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            workspace_catalog,
        )
        .get_workspace_catalog()
        .await;

        assert_eq!(
            result,
            Ok(WorkspaceCatalog {
                workspaces: vec![existing]
            })
        );

        let failing_catalog = FakeWorkspaceCatalog::empty()
            .with_list_error(WorkspaceSetupError::WorkspaceCatalogUnavailable);
        let failing_result = service(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            failing_catalog,
        )
        .get_workspace_catalog()
        .await;

        assert_eq!(
            failing_result,
            Err(WorkspaceSetupError::WorkspaceCatalogUnavailable)
        );
    }

    #[tokio::test]
    async fn get_provider_placement_options_rejects_missing_key_with_production_registry() {
        let result = WorkspaceSetupService::with_provider_registry(
            FakeCatalogReader::valid(),
            FakeSecretStore::missing_api_key(),
            FakeWorkspaceCatalog::empty(),
            WorkspaceSetupProviderRegistry::try_new().expect("registry initializes"),
        )
        .get_provider_placement_options(GpuCloudProviderId::Runpod)
        .await;

        assert_eq!(result, Err(WorkspaceSetupError::ProviderSetupIncomplete));
    }

    #[tokio::test]
    async fn get_provider_placement_options_rejects_missing_key_before_provider_selection() {
        let provider = FakePlacementOptionsProvider::with_result(Ok(
            placement_options_with_inventory(valid_inventory()),
        ));

        let result = service_with_provider(
            FakeCatalogReader::valid(),
            FakeSecretStore::missing_api_key(),
            FakeWorkspaceCatalog::empty(),
            provider.clone(),
        )
        .get_provider_placement_options(GpuCloudProviderId::Runpod)
        .await;

        assert_eq!(result, Err(WorkspaceSetupError::ProviderSetupIncomplete));
        assert_eq!(provider.calls(), Vec::<GpuCloudProviderId>::new());
    }

    #[tokio::test]
    async fn get_provider_placement_options_propagates_provider_errors() {
        let provider = FakePlacementOptionsProvider::with_result(Err(
            WorkspaceSetupError::ProviderApiUnavailable,
        ));

        let result = service_with_provider(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            FakeWorkspaceCatalog::empty(),
            provider.clone(),
        )
        .get_provider_placement_options(GpuCloudProviderId::Runpod)
        .await;

        assert_eq!(result, Err(WorkspaceSetupError::ProviderApiUnavailable));
        assert_eq!(provider.calls(), vec![GpuCloudProviderId::Runpod]);
    }

    #[tokio::test]
    async fn get_provider_placement_options_validates_fetched_inventory() {
        let invalid_provider = FakePlacementOptionsProvider::with_result(Ok(
            placement_options_with_inventory(ProviderInventory {
                fetched_at: " ".to_string(),
                ..valid_inventory()
            }),
        ));
        let invalid_result = service_with_provider(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            FakeWorkspaceCatalog::empty(),
            invalid_provider,
        )
        .get_provider_placement_options(GpuCloudProviderId::Runpod)
        .await;

        assert_eq!(
            invalid_result,
            Err(WorkspaceSetupError::ProviderInventoryInvalid)
        );

        let options = placement_options_with_inventory(valid_inventory());
        let valid_provider = FakePlacementOptionsProvider::with_result(Ok(options.clone()));
        let valid_result = service_with_provider(
            FakeCatalogReader::valid(),
            FakeSecretStore::with_api_key("rp_key_secret"),
            FakeWorkspaceCatalog::empty(),
            valid_provider,
        )
        .get_provider_placement_options(GpuCloudProviderId::Runpod)
        .await;

        assert_eq!(valid_result, Ok(options));
    }
}
