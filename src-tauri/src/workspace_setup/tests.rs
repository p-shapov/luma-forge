use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    bundled_catalog::reader::BundledCatalogReader,
    domain::{
        placement::PlacementPlan,
        profiles::{EndpointProfile, ProvisioningProfile},
        provider_inventory::ProviderInventory,
        provider_setup::{GpuCloudProviderId as DomainGpuCloudProviderId, ProviderApiKey},
        workspace::{Workspace, WorkspaceCatalog, WorkspaceLifecycleState},
    },
    provider_setup::ProviderSetupCoordinator,
    secrets::{SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_setup::contracts::CreateWorkspaceInput,
};

use super::*;

#[derive(Debug, Clone)]
struct MemorySecretStore {
    key: Arc<Mutex<Option<String>>>,
}

impl MemorySecretStore {
    fn with_key(key: &str) -> Self {
        Self {
            key: Arc::new(Mutex::new(Some(key.to_string()))),
        }
    }

    fn empty() -> Self {
        Self {
            key: Arc::new(Mutex::new(None)),
        }
    }

    fn clear_key(&self) {
        *self.key.lock().expect("secret lock") = None;
    }

    fn stored_key(&self) -> Option<String> {
        self.key.lock().expect("secret lock").clone()
    }
}

impl SecretStore for MemorySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(self.key.lock().expect("secret lock").is_some())
    }

    fn read_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        self.key
            .lock()
            .expect("secret lock")
            .clone()
            .map(ProviderApiKey::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("workspace setup tests do not replace secrets")
    }

    fn delete_api_key(
        &self,
        _provider_id: &DomainGpuCloudProviderId,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("workspace setup tests do not delete secrets")
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryProvider {
    fail: bool,
    setup_missing: bool,
    inventory: Option<ProviderInventory>,
}

impl ProviderInventoryGateway for MemoryProvider {
    fn fetch_inventory<'a>(
        &'a self,
        provider_id: &'a DomainGpuCloudProviderId,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInventory, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            if self.setup_missing {
                return Err(WorkspaceSetupError::ProviderSetupIncomplete);
            }
            if self.fail {
                return Err(WorkspaceSetupError::ProviderApiUnavailable);
            }
            Ok(self.inventory.clone().unwrap_or(ProviderInventory {
                gpu_cloud_provider_id: *provider_id,
                fetched_at: "2026-05-08T00:00:00Z".to_string(),
                max_persistent_storage_volume_size_bytes: None,
                datacenters: vec![],
            }))
        })
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryWorkspaceCatalog {
    workspaces: Arc<Mutex<Vec<Workspace>>>,
    fail_insert: bool,
    insert_delay: Option<Duration>,
    insert_started: Arc<AtomicBool>,
}

impl WorkspaceCatalogRepository for MemoryWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            Ok(WorkspaceCatalog {
                workspaces: self.workspaces.lock().expect("catalog lock").clone(),
            })
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            if self.fail_insert {
                return Err(WorkspaceSetupError::WorkspaceCatalogUnavailable);
            }
            self.insert_started.store(true, Ordering::SeqCst);
            if let Some(delay) = self.insert_delay {
                tokio::time::sleep(delay).await;
            }
            let mut workspaces = self.workspaces.lock().expect("catalog lock");
            if workspaces
                .iter()
                .any(|existing| existing.id == workspace.id)
            {
                return Err(WorkspaceSetupError::WorkspaceAlreadyExists);
            }
            workspaces.push(workspace.clone());
            Ok(workspace.clone())
        })
    }
}

impl MemoryWorkspaceCatalog {
    fn workspace_count(&self) -> usize {
        self.workspaces.lock().expect("catalog lock").len()
    }

    fn insert_started(&self) -> bool {
        self.insert_started.load(Ordering::SeqCst)
    }
}

fn service(
    secrets: MemorySecretStore,
    workspace_catalog: MemoryWorkspaceCatalog,
) -> WorkspaceSetupService<
    BundledCatalogReader,
    MemorySecretStore,
    MemoryProvider,
    MemoryWorkspaceCatalog,
> {
    WorkspaceSetupService::new(
        BundledCatalogReader,
        secrets,
        MemoryProvider::default(),
        workspace_catalog,
    )
}

pub(crate) fn sample_placement_plan() -> PlacementPlan {
    let reader = BundledCatalogReader;
    PlacementPlan::Runpod {
        selected_datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        persistent_storage_volume_size_bytes: 85899345920,
        selected_workflow_preset: reader
            .workflow_catalog()
            .expect("workflow catalog")
            .workflow_presets
            .remove(0),
        selected_provisioning_profile: reader
            .provisioning_profiles()
            .expect("provisioning profiles")
            .remove(0),
        selected_endpoint_profile: reader
            .endpoint_profiles()
            .expect("endpoint profiles")
            .remove(0),
    }
}

pub(crate) fn sample_workspace(id: &str) -> Workspace {
    Workspace {
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
        id: id.to_string(),
        name: "Workspace".to_string(),
        lifecycle_state: WorkspaceLifecycleState::Draft,
        placement_plan: sample_placement_plan(),
        persistent_storage_volume_snapshot: None,
        active_provisioning_pod_snapshot: None,
        serverless_endpoint_snapshot: None,
        last_provisioning_pod_snapshot: None,
        environment_prepared_at: None,
    }
}

fn create_workspace_request(id: &str) -> CreateWorkspaceInput {
    CreateWorkspaceInput {
        workspace_id: id.to_string(),
        name: "Workspace".to_string(),
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
        placement_plan: sample_placement_plan(),
    }
}

async fn create_workspace_with_gate(
    coordinator: Arc<ProviderSetupCoordinator>,
    service: WorkspaceSetupService<
        BundledCatalogReader,
        MemorySecretStore,
        MemoryProvider,
        MemoryWorkspaceCatalog,
    >,
    request: CreateWorkspaceInput,
) -> Result<Workspace, WorkspaceSetupError> {
    let provider_id = request.gpu_cloud_provider_id;
    let _guard = coordinator.lock(&provider_id).await;
    service.create_workspace(request).await
}

async fn wait_for_insert_started(catalog: &MemoryWorkspaceCatalog) {
    for _ in 0..50 {
        if catalog.insert_started() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    panic!("expected workspace insert to start");
}

#[test]
fn returns_catalogs() {
    let service = service(
        MemorySecretStore::empty(),
        MemoryWorkspaceCatalog::default(),
    );

    assert!(!service
        .get_workflow_catalog()
        .expect("workflow catalog")
        .workflow_presets
        .is_empty());
    assert!(!service
        .get_provisioning_profiles()
        .expect("profiles")
        .is_empty());
    assert!(!service
        .get_endpoint_profiles()
        .expect("profiles")
        .is_empty());
}

#[tokio::test]
async fn rejects_inventory_when_setup_is_missing() {
    let service = WorkspaceSetupService::new(
        BundledCatalogReader,
        MemorySecretStore::empty(),
        MemoryProvider {
            setup_missing: true,
            ..Default::default()
        },
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .get_provider_inventory(DomainGpuCloudProviderId::Runpod)
        .await
        .expect_err("missing key should fail");

    assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
}

#[tokio::test]
async fn maps_provider_inventory_failure() {
    let service = WorkspaceSetupService::new(
        BundledCatalogReader,
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryProvider {
            fail: true,
            ..Default::default()
        },
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .get_provider_inventory(DomainGpuCloudProviderId::Runpod)
        .await
        .expect_err("provider should fail");

    assert_eq!(error, WorkspaceSetupError::ProviderApiUnavailable);
}

#[tokio::test]
async fn maps_invalid_provider_inventory_to_provider_inventory_invalid() {
    let service = WorkspaceSetupService::new(
        BundledCatalogReader,
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryProvider {
            inventory: Some(ProviderInventory {
                gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
                fetched_at: " ".to_string(),
                max_persistent_storage_volume_size_bytes: None,
                datacenters: vec![],
            }),
            ..Default::default()
        },
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .get_provider_inventory(DomainGpuCloudProviderId::Runpod)
        .await
        .expect_err("invalid provider inventory should fail");

    assert_eq!(error, WorkspaceSetupError::ProviderInventoryInvalid);
}

#[tokio::test]
async fn creates_draft_workspace() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );

    let response = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: " Workspace ".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect("workspace should create");

    assert_eq!(response.name, "Workspace");
    assert_eq!(response.lifecycle_state, WorkspaceLifecycleState::Draft);
    assert!(response.persistent_storage_volume_snapshot.is_none());
    assert!(response.active_provisioning_pod_snapshot.is_none());
    assert!(response.serverless_endpoint_snapshot.is_none());
}

#[tokio::test]
async fn rejects_invalid_workspace_id() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "not-a-uuid".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect_err("invalid workspace id should fail");

    assert_eq!(error, WorkspaceSetupError::InvalidWorkspaceId);
}

#[tokio::test]
async fn rejects_missing_workspace_name() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: " ".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect_err("missing workspace name should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceNameRequired);
}

#[tokio::test]
async fn rejects_invalid_stored_provider_key_during_workspace_creation() {
    let service = service(
        MemorySecretStore::with_key(" "),
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect_err("invalid stored provider key should fail");

    assert_eq!(error, WorkspaceSetupError::StoredProviderApiKeyInvalid);
}

#[tokio::test]
async fn workspace_creation_waits_for_provider_setup_deletion() {
    let secrets = MemorySecretStore::with_key("rp_123_secret");
    let catalog = MemoryWorkspaceCatalog::default();
    let coordinator = Arc::new(ProviderSetupCoordinator::default());
    let provider_id = DomainGpuCloudProviderId::Runpod;
    let delete_guard = coordinator.lock(&provider_id).await;

    let create = tokio::spawn(create_workspace_with_gate(
        coordinator.clone(),
        service(secrets.clone(), catalog.clone()),
        create_workspace_request("018f6a40-0000-7000-8000-000000000001"),
    ));
    tokio::time::sleep(Duration::from_millis(10)).await;
    secrets.clear_key();
    drop(delete_guard);

    let error = create
        .await
        .expect("create task should join")
        .expect_err("workspace creation should see deleted setup");

    assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
    assert_eq!(catalog.workspace_count(), 0);
    assert_eq!(secrets.stored_key(), None);
}

#[tokio::test]
async fn provider_setup_deletion_waits_for_workspace_creation_persistence() {
    let secrets = MemorySecretStore::with_key("rp_123_secret");
    let catalog = MemoryWorkspaceCatalog {
        insert_delay: Some(Duration::from_millis(50)),
        ..Default::default()
    };
    let coordinator = Arc::new(ProviderSetupCoordinator::default());

    let create = tokio::spawn(create_workspace_with_gate(
        coordinator.clone(),
        service(secrets.clone(), catalog.clone()),
        create_workspace_request("018f6a40-0000-7000-8000-000000000001"),
    ));
    wait_for_insert_started(&catalog).await;

    let delete = tokio::spawn({
        let coordinator = coordinator.clone();
        let secrets = secrets.clone();
        async move {
            let provider_id = DomainGpuCloudProviderId::Runpod;
            let _guard = coordinator.lock(&provider_id).await;
            secrets.clear_key();
        }
    });

    create
        .await
        .expect("create task should join")
        .expect("workspace creation should finish before deletion");
    delete.await.expect("delete task should join");

    assert_eq!(catalog.workspace_count(), 1);
    assert_eq!(secrets.stored_key(), None);
}

#[tokio::test]
async fn rejects_duplicate_workspace_id() {
    let workspace_catalog = MemoryWorkspaceCatalog::default();
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        workspace_catalog,
    );
    let request = CreateWorkspaceInput {
        workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
        name: "Workspace".to_string(),
        gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
        placement_plan: sample_placement_plan(),
    };

    service
        .create_workspace(request.clone())
        .await
        .expect("first create");
    let error = service
        .create_workspace(request)
        .await
        .expect_err("duplicate should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceAlreadyExists);
}

#[tokio::test]
async fn rejects_missing_datacenter_selection() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        ..
    } = &mut placement_plan;
    selected_datacenter_id.clear();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("missing datacenter should fail");

    assert_eq!(error, WorkspaceSetupError::PlacementDatacenterRequired);
}

#[tokio::test]
async fn rejects_missing_gpu_selection() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_gpu_id, ..
    } = &mut placement_plan;
    selected_gpu_id.clear();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("missing GPU should fail");

    assert_eq!(error, WorkspaceSetupError::PlacementGpuRequired);
}

#[tokio::test]
async fn rejects_stale_workflow_preset() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_workflow_preset,
        ..
    } = &mut placement_plan;
    selected_workflow_preset.name = "Changed".to_string();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("stale preset should fail");

    assert_eq!(error, WorkspaceSetupError::WorkflowPresetStale);
}

#[tokio::test]
async fn rejects_stale_provisioning_profile() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_provisioning_profile,
        ..
    } = &mut placement_plan;
    let ProvisioningProfile::Runpod { name, .. } = selected_provisioning_profile;
    *name = "Changed".to_string();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("stale provisioning profile should fail");

    assert_eq!(error, WorkspaceSetupError::ProvisioningProfileStale);
}

#[tokio::test]
async fn rejects_stale_endpoint_profile() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_endpoint_profile,
        ..
    } = &mut placement_plan;
    let EndpointProfile::Runpod { name, .. } = selected_endpoint_profile;
    *name = "Changed".to_string();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("stale endpoint profile should fail");

    assert_eq!(error, WorkspaceSetupError::EndpointProfileStale);
}

#[tokio::test]
async fn rejects_insufficient_storage() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        persistent_storage_volume_size_bytes,
        ..
    } = &mut placement_plan;
    *persistent_storage_volume_size_bytes = 1;

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("small storage should fail");

    assert_eq!(error, WorkspaceSetupError::StorageSizeBelowPresetMinimum);
}

#[tokio::test]
async fn rejects_unknown_workflow_preset() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let PlacementPlan::Runpod {
        selected_workflow_preset,
        ..
    } = &mut placement_plan;
    selected_workflow_preset.id = "unknown".to_string();

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("unknown workflow preset should fail");

    assert_eq!(error, WorkspaceSetupError::WorkflowPresetStale);
}

#[tokio::test]
async fn maps_persistence_failure() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog {
            workspaces: Arc::default(),
            fail_insert: true,
            ..Default::default()
        },
    );

    let error = service
        .create_workspace(CreateWorkspaceInput {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect_err("insert should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}
