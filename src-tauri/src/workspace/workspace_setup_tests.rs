use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    bundled::{
        bundled_catalog::{BundledCatalogReader, CatalogReader},
        bundled_contracts::EndpointProfile,
    },
    domain::{
        provider_inventory::ProviderInventory,
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workflow::WorkflowExecutionType,
        workspace::WorkspaceLifecycleState,
    },
    provider_setup::ProviderSetupError,
    secrets::SecretStore,
    workspace::{
        workspace_catalog::WorkspaceCatalogRepository,
        workspace_contracts::{PlacementPlan, Workspace, WorkspaceCatalog},
    },
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
}

impl SecretStore for MemorySecretStore {
    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, ProviderSetupError> {
        self.key
            .lock()
            .expect("secret lock")
            .clone()
            .map(ProviderApiKey::new)
            .transpose()
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), ProviderSetupError> {
        unimplemented!("workspace setup tests do not replace secrets")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), ProviderSetupError> {
        unimplemented!("workspace setup tests do not delete secrets")
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryProvider {
    fail: bool,
}

impl ProviderInventoryGateway for MemoryProvider {
    fn fetch_inventory<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        _api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInventory, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            if self.fail {
                return Err(WorkspaceSetupError::ProviderApiUnavailable);
            }
            Ok(ProviderInventory {
                gpu_cloud_provider_id: *provider_id,
                fetched_at: "2026-05-08T00:00:00Z".to_string(),
                max_persistent_storage_volume_size_bytes: None,
                datacenters: vec![],
            })
        })
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryWorkspaceCatalog {
    workspaces: Arc<Mutex<Vec<Workspace>>>,
    fail_insert: bool,
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
    PlacementPlan {
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
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
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

#[test]
fn returns_catalogs() {
    let service = service(
        MemorySecretStore::empty(),
        MemoryWorkspaceCatalog::default(),
    );

    assert!(!service
        .get_workflow_catalog()
        .expect("workflow catalog")
        .workflow_catalog
        .workflow_presets
        .is_empty());
    assert!(!service
        .get_provisioning_profiles()
        .expect("profiles")
        .provisioning_profiles
        .is_empty());
    assert!(!service
        .get_endpoint_profiles()
        .expect("profiles")
        .endpoint_profiles
        .is_empty());
}

#[tokio::test]
async fn rejects_inventory_when_setup_is_missing() {
    let service = service(
        MemorySecretStore::empty(),
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .get_provider_inventory(GetProviderInventoryRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect_err("missing key should fail");

    assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
}

#[tokio::test]
async fn maps_provider_inventory_failure() {
    let service = WorkspaceSetupService::new(
        BundledCatalogReader,
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryProvider { fail: true },
        MemoryWorkspaceCatalog::default(),
    );

    let error = service
        .get_provider_inventory(GetProviderInventoryRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        })
        .await
        .expect_err("provider should fail");

    assert_eq!(error, WorkspaceSetupError::ProviderApiUnavailable);
}

#[tokio::test]
async fn creates_draft_workspace() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );

    let response = service
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: " Workspace ".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect("workspace should create");

    assert_eq!(response.workspace.name, "Workspace");
    assert_eq!(
        response.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert!(response
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert!(response
        .workspace
        .active_provisioning_pod_snapshot
        .is_none());
    assert!(response.workspace.serverless_endpoint_snapshot.is_none());
}

#[tokio::test]
async fn rejects_duplicate_workspace_id() {
    let workspace_catalog = MemoryWorkspaceCatalog::default();
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        workspace_catalog,
    );
    let request = CreateWorkspaceRequest {
        workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
        name: "Workspace".to_string(),
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
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
async fn rejects_stale_catalog_object() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    placement_plan.selected_workflow_preset.name = "Changed".to_string();

    let error = service
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("stale preset should fail");

    assert_eq!(error, WorkspaceSetupError::InvalidPlacementPlan);
}

#[tokio::test]
async fn rejects_insufficient_storage() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    placement_plan.persistent_storage_volume_size_bytes = 1;

    let error = service
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("small storage should fail");

    assert_eq!(error, WorkspaceSetupError::InvalidPlacementPlan);
}

#[tokio::test]
async fn rejects_incompatible_endpoint_profile() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog::default(),
    );
    let mut placement_plan = sample_placement_plan();
    let EndpointProfile::Runpod {
        workflow_execution_type,
        ..
    } = &mut placement_plan.selected_endpoint_profile;
    *workflow_execution_type = WorkflowExecutionType::T2i;
    placement_plan.selected_workflow_preset.id = "unknown".to_string();

    let error = service
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan,
        })
        .await
        .expect_err("incompatible request should fail");

    assert_eq!(error, WorkspaceSetupError::InvalidPlacementPlan);
}

#[tokio::test]
async fn maps_persistence_failure() {
    let service = service(
        MemorySecretStore::with_key("rp_123_secret"),
        MemoryWorkspaceCatalog {
            workspaces: Arc::default(),
            fail_insert: true,
        },
    );

    let error = service
        .create_workspace(CreateWorkspaceRequest {
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            name: "Workspace".to_string(),
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            placement_plan: sample_placement_plan(),
        })
        .await
        .expect_err("insert should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}
