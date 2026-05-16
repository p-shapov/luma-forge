use super::*;

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

use super::gateways::ProvisionerWorkerGateway;

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
            ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot,
            Workspace, WorkspaceCatalog, WorkspaceLifecycleState, WorkspaceProvisioningFailureCode,
            WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
            WorkspaceProvisioningRecoveryAction,
        },
    },
    provisioner_worker::{
        ProvisionerWorkerJobStatus, ProvisionerWorkerStartRequest, ProvisionerWorkerStatus,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_setup::{error::WorkspaceSetupError, tests::sample_workspace},
};

#[derive(Debug, Clone)]
struct MemorySecretStore {
    api_key: Option<String>,
    worker_tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl SecretStore for MemorySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(self.api_key.is_some())
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        self.api_key
            .clone()
            .map(ProviderApiKey::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provisioning tests do not replace provider keys")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("provisioning tests do not delete provider keys")
    }

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .insert(workspace_id.to_string(), token.expose_secret().to_string());
        Ok(())
    }

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .get(workspace_id)
            .cloned()
            .map(ProvisionerWorkerBearerToken::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProvisionerWorkerToken)
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .remove(workspace_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryWorkspaceCatalog {
    workspaces: Arc<Mutex<Vec<Workspace>>>,
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

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            Ok(self
                .workspaces
                .lock()
                .expect("catalog lock")
                .iter()
                .find(|workspace| workspace.id == id)
                .cloned())
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            self.workspaces
                .lock()
                .expect("catalog lock")
                .push(workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            let mut workspaces = self.workspaces.lock().expect("catalog lock");
            let existing = workspaces
                .iter_mut()
                .find(|existing| existing.id == workspace.id)
                .ok_or(WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
            *existing = workspace.clone();
            Ok(workspace.clone())
        })
    }
}

#[derive(Debug, Clone, Default)]
struct FakeProvider {
    create_volume_count: Arc<AtomicUsize>,
    get_volume_count: Arc<AtomicUsize>,
    delete_volume_count: Arc<AtomicUsize>,
    discover_volumes_count: Arc<AtomicUsize>,
    discover_pods_count: Arc<AtomicUsize>,
    create_pod_count: Arc<AtomicUsize>,
    create_pod_inputs: Arc<Mutex<Vec<CreateProvisioningPodInput>>>,
    get_pod_count: Arc<AtomicUsize>,
    delete_pod_count: Arc<AtomicUsize>,
    discover_templates_count: Arc<AtomicUsize>,
    create_template_count: Arc<AtomicUsize>,
    get_template_count: Arc<AtomicUsize>,
    delete_template_count: Arc<AtomicUsize>,
    discover_endpoints_count: Arc<AtomicUsize>,
    create_endpoint_count: Arc<AtomicUsize>,
    get_endpoint_count: Arc<AtomicUsize>,
    delete_endpoint_count: Arc<AtomicUsize>,
    create_volume_error: Option<WorkspaceProvisioningError>,
    create_pod_error: Option<WorkspaceProvisioningError>,
    create_template_error: Option<WorkspaceProvisioningError>,
    create_endpoint_error: Option<WorkspaceProvisioningError>,
    get_volume_error: Option<WorkspaceProvisioningError>,
    get_pod_error: Option<WorkspaceProvisioningError>,
    get_template_error: Option<WorkspaceProvisioningError>,
    get_endpoint_error: Option<WorkspaceProvisioningError>,
    discovered_volumes: Vec<NetworkVolumeObservation>,
    discovered_pods: Vec<ProvisioningPodObservation>,
    subsequent_discovered_pods: Option<Vec<ProvisioningPodObservation>>,
    discovered_templates: Vec<EndpointTemplateObservation>,
    discovered_endpoints: Vec<ServerlessEndpointObservation>,
    get_volume_status: Option<ProviderResourceStatus>,
    get_pod_status_url: Option<Option<String>>,
    get_template_status: Option<ProviderResourceStatus>,
    get_endpoint_status: Option<ProviderResourceStatus>,
    delete_endpoint_error: Option<WorkspaceProvisioningError>,
}

impl ProviderProvisioningGateway for FakeProvider {
    fn create_network_volume<'a>(
        &'a self,
        _input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.create_volume_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.create_volume_error {
                return Err(error.clone());
            }
            Ok(NetworkVolumeObservation {
                provider_resource_id: "volume-1".to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
                provider_resource_status: ProviderResourceStatus::Ready,
                mount_path: "/workspace".to_string(),
            })
        })
    }

    fn get_network_volume<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.get_volume_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.get_volume_error {
                return Err(error.clone());
            }
            Ok(NetworkVolumeObservation {
                provider_resource_id: _volume_id.to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
                provider_resource_status: self
                    .get_volume_status
                    .clone()
                    .unwrap_or(ProviderResourceStatus::Ready),
                mount_path: "/workspace".to_string(),
            })
        })
    }

    fn discover_network_volumes<'a>(
        &'a self,
        _input: DiscoverNetworkVolumesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<NetworkVolumeObservation>, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.discover_volumes_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.discovered_volumes.clone())
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_volume_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.create_pod_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.create_pod_error {
                return Err(error.clone());
            }
            self.create_pod_inputs
                .lock()
                .expect("create pod inputs")
                .push(input.clone());
            assert_eq!(
                input.provisioner_worker_image_ref,
                "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:af202cfc2a2b97ec970925d7e2d5abd5e9fb6a2dd5bededbf364f834bc0ab201"
            );
            assert_eq!(input.provisioner_worker_port, 8080);
            Ok(ProvisioningPodObservation {
                provider_resource_id: "pod-1".to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                selected_gpu_id: "NVIDIA RTX 4090".to_string(),
                provider_resource_status: ProviderResourceStatus::Running,
                provisioner_status_url: Some("http://203.0.113.10:30001/status".to_string()),
            })
        })
    }

    fn discover_provisioning_pods<'a>(
        &'a self,
        _input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let count = self.discover_pods_count.fetch_add(1, Ordering::SeqCst);
            if count > 0 {
                if let Some(discovered_pods) = &self.subsequent_discovered_pods {
                    return Ok(discovered_pods.clone());
                }
            }
            Ok(self.discovered_pods.clone())
        })
    }

    fn get_provisioning_pod<'a>(
        &'a self,
        input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.get_pod_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.get_pod_error {
                return Err(error.clone());
            }
            Ok(ProvisioningPodObservation {
                provider_resource_id: input.provider_resource_id,
                datacenter_id: input.datacenter_id,
                selected_gpu_id: input.selected_gpu_id,
                provider_resource_status: ProviderResourceStatus::Running,
                provisioner_status_url: self
                    .get_pod_status_url
                    .clone()
                    .unwrap_or_else(|| Some("http://203.0.113.10:30001/status".to_string())),
            })
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_pod_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn create_endpoint_template<'a>(
        &'a self,
        input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.create_template_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.create_template_error {
                return Err(error.clone());
            }
            assert_eq!(input.endpoint_worker_port, 8080);
            Ok(EndpointTemplateObservation {
                template_id: "template-1".to_string(),
                endpoint_worker_image_ref: "ghcr.io/luma-forge/endpoint-worker:test".to_string(),
                mount_path: "/workspace".to_string(),
                provider_resource_status: ProviderResourceStatus::Ready,
            })
        })
    }

    fn get_endpoint_template<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.get_template_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.get_template_error {
                return Err(error.clone());
            }
            Ok(EndpointTemplateObservation {
                template_id: _template_id.to_string(),
                endpoint_worker_image_ref: "ghcr.io/luma-forge/endpoint-worker:test".to_string(),
                mount_path: "/workspace".to_string(),
                provider_resource_status: self
                    .get_template_status
                    .clone()
                    .unwrap_or(ProviderResourceStatus::Ready),
            })
        })
    }

    fn discover_endpoint_templates<'a>(
        &'a self,
        _input: DiscoverEndpointTemplatesInput,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Vec<EndpointTemplateObservation>, WorkspaceProvisioningError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.discover_templates_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.discovered_templates.clone())
        })
    }

    fn delete_endpoint_template<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_template_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        _input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.create_endpoint_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.create_endpoint_error {
                return Err(error.clone());
            }
            Ok(ServerlessEndpointObservation {
                provider_resource_id: "endpoint-1".to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                selected_gpu_id: "NVIDIA RTX 4090".to_string(),
                provider_resource_status: ProviderResourceStatus::Ready,
                endpoint_invoke_url: "https://api.runpod.ai/v2/endpoint-1/runsync".to_string(),
            })
        })
    }

    fn get_serverless_endpoint<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.get_endpoint_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.get_endpoint_error {
                return Err(error.clone());
            }
            Ok(ServerlessEndpointObservation {
                provider_resource_id: _endpoint_id.to_string(),
                datacenter_id: "EU-RO-1".to_string(),
                selected_gpu_id: "NVIDIA RTX 4090".to_string(),
                provider_resource_status: self
                    .get_endpoint_status
                    .clone()
                    .unwrap_or(ProviderResourceStatus::Ready),
                endpoint_invoke_url: "https://api.runpod.ai/v2/endpoint-1/runsync".to_string(),
            })
        })
    }

    fn discover_serverless_endpoints<'a>(
        &'a self,
        _input: DiscoverServerlessEndpointsInput,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Vec<ServerlessEndpointObservation>, WorkspaceProvisioningError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.discover_endpoints_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.discovered_endpoints.clone())
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceProvisioningError>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_endpoint_count.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = &self.delete_endpoint_error {
                return Err(error.clone());
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone)]
struct FakeWorker {
    start_count: Arc<AtomicUsize>,
    start_requests: Arc<Mutex<Vec<ProvisionerWorkerStartRequest>>>,
    status: Arc<Mutex<ProvisionerWorkerStatus>>,
    status_error: Option<WorkspaceProvisioningError>,
}

impl FakeWorker {
    fn idle() -> Self {
        Self::with_status(ProvisionerWorkerJobStatus::Idle)
    }

    fn succeeded() -> Self {
        Self::with_status(ProvisionerWorkerJobStatus::Succeeded)
    }

    fn with_status(status: ProvisionerWorkerJobStatus) -> Self {
        Self {
            start_count: Arc::default(),
            start_requests: Arc::default(),
            status: Arc::new(Mutex::new(ProvisionerWorkerStatus {
                phase: match status {
                    ProvisionerWorkerJobStatus::Succeeded => {
                        crate::provisioner_worker::ProvisionerWorkerPhase::Completed
                    }
                    _ => crate::provisioner_worker::ProvisionerWorkerPhase::Idle,
                },
                status,
                progress_percent: None,
                diagnostic: None,
            })),
            status_error: None,
        }
    }

    fn with_status_error(error: WorkspaceProvisioningError) -> Self {
        Self {
            status_error: Some(error),
            ..Self::idle()
        }
    }
}

impl ProvisionerWorkerGateway for FakeWorker {
    fn start<'a>(
        &'a self,
        _provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.start_count.fetch_add(1, Ordering::SeqCst);
            self.start_requests
                .lock()
                .expect("start requests")
                .push(request.clone());
            Ok(ProvisionerWorkerStatus {
                status: ProvisionerWorkerJobStatus::Running,
                phase: crate::provisioner_worker::ProvisionerWorkerPhase::InstallingRuntime,
                progress_percent: Some(25),
                diagnostic: None,
            })
        })
    }

    fn status<'a>(
        &'a self,
        _provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, WorkspaceProvisioningError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            if let Some(error) = &self.status_error {
                return Err(error.clone());
            }
            Ok(self.status.lock().expect("worker status").clone())
        })
    }
}

#[tokio::test]
async fn initiate_transitions_draft_to_provisioning() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service(catalog.clone(), FakeProvider::default());

    let result = service.initiate(&workspace.id).await.expect("initiate");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert_eq!(
        catalog
            .find_workspace_by_id(&workspace.id)
            .await
            .expect("find")
            .expect("workspace")
            .lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
}

#[tokio::test]
async fn sync_creates_network_volume_once() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);

    service.sync(&workspace.id).await.expect("second sync");
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_refreshes_existing_volume_snapshot() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Creating,
        provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result
            .workspace
            .persistent_storage_volume_snapshot
            .expect("volume")
            .provider_resource_status,
        ProviderResourceStatus::Ready
    );
    assert_eq!(provider.get_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_failed_when_volume_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Creating,
        provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_volume_status: Some(ProviderResourceStatus::Failed),
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProviderResourceFailed
    );
    assert_eq!(failure.phase, WorkspaceProvisioningPhase::CreatingVolume);
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProviderResource
    );
    assert_eq!(
        failure.recovery_action,
        WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources
    );
    assert_eq!(provider.get_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn indeterminate_volume_create_marks_failed_without_losing_workspace() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_volume_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let service = service(catalog, provider);

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
    );
}

#[tokio::test]
async fn provider_command_failure_preserves_workspace_metadata() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_volume_error: Some(WorkspaceProvisioningError::ProviderRateLimited),
        ..Default::default()
    };
    let service = service(catalog.clone(), provider);

    let error = service
        .sync(&workspace.id)
        .await
        .expect_err("rate limiting should be a command error");

    assert_eq!(error, WorkspaceProvisioningError::ProviderRateLimited);
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.persistent_storage_volume_snapshot.is_none());
    assert!(stored.last_provisioning_failure.is_none());
}

#[tokio::test]
async fn sync_creates_provisioning_pod_and_stores_worker_token() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker_tokens = Arc::new(Mutex::new(HashMap::new()));
    let service = WorkspaceProvisioningService::new(
        MemorySecretStore {
            api_key: Some("rp_123_secret".to_string()),
            worker_tokens: worker_tokens.clone(),
        },
        provider.clone(),
        catalog,
        FakeWorker::idle(),
        WorkspaceProvisioningCoordinator::default(),
        test_config(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
    let create_pod_inputs = provider
        .create_pod_inputs
        .lock()
        .expect("create pod inputs");
    assert_eq!(create_pod_inputs.len(), 1);
    assert_eq!(
        create_pod_inputs[0].provisioner_worker_image_ref,
        workspace
            .resolved_runtime_implementation
            .provisioner_image_ref
    );
    assert_eq!(worker_tokens.lock().expect("tokens").len(), 1);
    drop(create_pod_inputs);

    service.sync(&workspace.id).await.expect("second sync");
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_adopts_single_discovered_provisioning_pod_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_pods: vec![discovered_pod("pod-existing")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    let active_pod = result
        .workspace
        .active_provisioning_pod_snapshot
        .expect("active pod should be adopted");
    assert_eq!(active_pod.provider_resource_id, "pod-existing");
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn indeterminate_pod_create_recovery_adopts_discovered_pod() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_pod_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        discovered_pods: Vec::new(),
        subsequent_discovered_pods: Some(vec![discovered_pod("pod-existing")]),
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    let active_pod = result
        .workspace
        .active_provisioning_pod_snapshot
        .expect("discovered pod should be tracked after indeterminate create");
    assert_eq!(active_pod.provider_resource_id, "pod-existing");
    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 2);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_fails_closed_when_multiple_discovered_provisioning_pods_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_pods: vec![discovered_pod("pod-1"), discovered_pod("pod-2")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
    );
    assert_eq!(
        failure.phase,
        WorkspaceProvisioningPhase::StartingProvisioningPod
    );
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn concurrent_sync_is_read_only() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let coordinator = WorkspaceProvisioningCoordinator::default();
    let _guard = coordinator.try_enter(&workspace.id).expect("enter");
    let service = WorkspaceProvisioningService::new(
        MemorySecretStore {
            api_key: Some("rp_123_secret".to_string()),
            worker_tokens: Arc::default(),
        },
        provider.clone(),
        catalog,
        FakeWorker::idle(),
        coordinator,
        test_config(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_starts_idle_worker_and_returns_worker_progress() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens);

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.get_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(worker.start_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.progress.phase,
        WorkspaceProvisioningPhase::PreparingEnvironment
    );
    assert_eq!(result.progress.percent, Some(25));
    assert!(result.workspace.environment_prepared_at.is_none());
}

#[tokio::test]
async fn sync_starts_idle_worker_with_job_id() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let worker = FakeWorker::idle();
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        worker.clone(),
        worker_token_map(&workspace.id),
    );

    service.sync(&workspace.id).await.expect("sync");

    let requests = worker.start_requests.lock().expect("start requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].job_id, workspace.id);
    assert_eq!(
        requests[0].workflow_preset.id,
        workspace.placement_plan.selected_workflow_preset().id
    );
}

#[tokio::test]
async fn sync_treats_temporarily_unavailable_worker_as_running_progress() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker =
        FakeWorker::with_status_error(WorkspaceProvisioningError::ProvisionerWorkerUnavailable);
    let service = service_with_parts(
        catalog.clone(),
        provider.clone(),
        worker.clone(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert_eq!(
        result.progress.phase,
        WorkspaceProvisioningPhase::PreparingEnvironment
    );
    assert_eq!(worker.start_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.last_provisioning_failure.is_none());
}

#[tokio::test]
async fn sync_preserves_existing_provisioner_status_url_when_provider_omits_it() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    let mut active_pod = active_pod();
    active_pod.provider_resource_status = ProviderResourceStatus::Creating;
    workspace.active_provisioning_pod_snapshot = Some(active_pod.clone());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_pod_status_url: Some(None),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider,
        FakeWorker::idle(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result
            .workspace
            .active_provisioning_pod_snapshot
            .expect("active pod")
            .provisioner_status_url,
        active_pod.provisioner_status_url
    );
}

#[tokio::test]
async fn worker_response_invalid_persists_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::with_status_error(
            WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid {
                diagnostic: Some(
                    "code: invalid_request\nreason_code: missing_job_id\nmessage: job_id is required"
                        .to_string(),
                ),
            },
        ),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid
    );
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProvisionerWorker
    );
    assert_eq!(
        failure.diagnostic,
        Some(
            "code: invalid_request\nreason_code: missing_job_id\nmessage: job_id is required"
                .to_string()
        )
    );
}

#[tokio::test]
async fn worker_terminal_failure_persists_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::with_status_error(WorkspaceProvisioningError::ProvisionerWorkerFailed {
            diagnostic: Some("safe diagnostic".to_string()),
        }),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed
    );
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProvisionerWorker
    );
    assert_eq!(failure.diagnostic.as_deref(), Some("safe diagnostic"));
}

#[tokio::test]
async fn missing_worker_token_marks_failed_with_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .progress
            .failure
            .expect("progress should expose failure")
            .code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing
    );
}

#[tokio::test]
async fn sync_persists_environment_timestamp_when_worker_succeeds() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::succeeded(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result.workspace.environment_prepared_at.is_some());
    assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
}

#[tokio::test]
async fn sync_terminates_pod_and_deletes_token_after_environment_is_prepared() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        tokens.clone(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    assert!(result.workspace.last_provisioning_pod_snapshot.is_some());
    assert!(tokens.lock().expect("tokens").is_empty());
}

#[tokio::test]
async fn sync_creates_endpoint_template_after_environment_preparation() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 1);
    assert!(runpod_template_snapshot(&result.workspace).is_some());
}

#[tokio::test]
async fn sync_creates_endpoint_from_ready_template_and_keep_alive_plan() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 1);
    assert!(result.workspace.serverless_endpoint_snapshot.is_some());
}

#[tokio::test]
async fn sync_marks_failed_when_template_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Creating)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_template_status: Some(ProviderResourceStatus::Terminated),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderResourceTerminated
    );
    assert_eq!(provider.get_template_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_failed_when_endpoint_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
        provider_resource_status: ProviderResourceStatus::Creating,
        ..endpoint_snapshot()
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_endpoint_status: Some(ProviderResourceStatus::Unknown),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderResourceUnknown
    );
    assert_eq!(provider.get_endpoint_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_workspace_ready_after_required_snapshots_are_ready() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Ready
    );
}

#[tokio::test]
async fn cancel_cleans_known_resources_and_returns_workspace_to_draft() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens.clone());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert!(result.workspace.provider_provisioning_snapshot.is_none());
}

#[tokio::test]
async fn cancel_skips_worker_cancel_when_provider_cleanup_succeeds() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens.clone());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert!(result.workspace.last_provisioning_failure.is_none());
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert!(result.workspace.provider_provisioning_snapshot.is_none());
}

#[tokio::test]
async fn cancel_marks_failed_and_preserves_metadata_when_cleanup_fails() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        delete_endpoint_error: Some(WorkspaceProvisioningError::ProviderApiUnavailable),
        ..Default::default()
    };
    let service = service_with_parts(catalog, provider, FakeWorker::idle(), Arc::default());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::CancellationCleanupFailed
    );
    assert!(result.workspace.serverless_endpoint_snapshot.is_some());
    assert!(result.workspace.provider_provisioning_snapshot.is_some());
}

#[tokio::test]
async fn sync_adopts_single_discovered_network_volume_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_volumes: vec![discovered_volume("volume-existing")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    let volume = result
        .workspace
        .persistent_storage_volume_snapshot
        .expect("volume should be adopted");
    assert_eq!(volume.provider_resource_id, "volume-existing");
    assert_eq!(provider.discover_volumes_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_closed_when_multiple_discovered_network_volumes_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_volumes: vec![discovered_volume("volume-1"), discovered_volume("volume-2")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_adopts_single_discovered_endpoint_template_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_templates: vec![discovered_template("template-existing")],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    let template = runpod_template_snapshot(&result.workspace).expect("template should be adopted");
    assert_eq!(template.template_id, "template-existing");
    assert_eq!(provider.discover_templates_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_closed_when_multiple_discovered_endpoint_templates_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_templates: vec![
            discovered_template("template-1"),
            discovered_template("template-2"),
        ],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_adopts_single_discovered_serverless_endpoint_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_endpoints: vec![discovered_endpoint("endpoint-existing")],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    let endpoint = result
        .workspace
        .serverless_endpoint_snapshot
        .expect("endpoint should be adopted");
    assert_eq!(endpoint.provider_resource_id, "endpoint-existing");
    assert_eq!(provider.discover_endpoints_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_closed_when_multiple_discovered_serverless_endpoints_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_endpoints: vec![
            discovered_endpoint("endpoint-1"),
            discovered_endpoint("endpoint-2"),
        ],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn indeterminate_create_failures_do_not_retry_create_on_next_sync() {
    let volume_catalog = MemoryWorkspaceCatalog::default();
    let mut volume_workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    volume_workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    volume_catalog
        .insert_workspace(&volume_workspace)
        .await
        .expect("insert");
    let volume_provider = FakeProvider {
        create_volume_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let volume_service = service(volume_catalog, volume_provider.clone());
    let volume_result = volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &volume_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        volume_provider.create_volume_count.load(Ordering::SeqCst),
        1
    );

    let pod_catalog = MemoryWorkspaceCatalog::default();
    let pod_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000002");
    pod_catalog
        .insert_workspace(&pod_workspace)
        .await
        .expect("insert");
    let pod_provider = FakeProvider {
        create_pod_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let pod_service = service(pod_catalog, pod_provider.clone());
    let pod_result = pod_service.sync(&pod_workspace.id).await.expect("sync");
    assert_failure(
        &pod_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    pod_service
        .sync(&pod_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(pod_provider.create_pod_count.load(Ordering::SeqCst), 1);

    let template_catalog = MemoryWorkspaceCatalog::default();
    let mut template_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000003");
    template_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    template_catalog
        .insert_workspace(&template_workspace)
        .await
        .expect("insert");
    let template_provider = FakeProvider {
        create_template_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let template_service = service_with_parts(
        template_catalog,
        template_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let template_result = template_service
        .sync(&template_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &template_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    template_service
        .sync(&template_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        template_provider
            .create_template_count
            .load(Ordering::SeqCst),
        1
    );

    let endpoint_catalog = MemoryWorkspaceCatalog::default();
    let mut endpoint_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000004");
    endpoint_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    endpoint_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
        });
    endpoint_catalog
        .insert_workspace(&endpoint_workspace)
        .await
        .expect("insert");
    let endpoint_provider = FakeProvider {
        create_endpoint_error: Some(WorkspaceProvisioningError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let endpoint_service = service_with_parts(
        endpoint_catalog,
        endpoint_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let endpoint_result = endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &endpoint_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        endpoint_provider
            .create_endpoint_count
            .load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn missing_tracked_resources_mark_workspace_failed_without_recreate() {
    let volume_catalog = MemoryWorkspaceCatalog::default();
    let mut volume_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    volume_workspace
        .persistent_storage_volume_snapshot
        .as_mut()
        .expect("volume")
        .provider_resource_status = ProviderResourceStatus::Creating;
    volume_catalog
        .insert_workspace(&volume_workspace)
        .await
        .expect("insert");
    let volume_provider = FakeProvider {
        get_volume_error: Some(WorkspaceProvisioningError::ProviderResourceNotFound),
        ..Default::default()
    };
    let volume_service = service(volume_catalog, volume_provider.clone());
    let volume_result = volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &volume_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert!(volume_result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert_eq!(
        volume_provider.create_volume_count.load(Ordering::SeqCst),
        0
    );

    let pod_catalog = MemoryWorkspaceCatalog::default();
    let mut pod_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000002");
    pod_workspace.active_provisioning_pod_snapshot = Some(active_pod());
    pod_catalog
        .insert_workspace(&pod_workspace)
        .await
        .expect("insert");
    let pod_provider = FakeProvider {
        get_pod_error: Some(WorkspaceProvisioningError::ProviderResourceNotFound),
        ..Default::default()
    };
    let pod_service = service(pod_catalog, pod_provider.clone());
    let pod_result = pod_service.sync(&pod_workspace.id).await.expect("sync");
    assert_failure(
        &pod_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    assert!(pod_result
        .workspace
        .active_provisioning_pod_snapshot
        .is_some());
    assert_eq!(pod_provider.create_pod_count.load(Ordering::SeqCst), 0);

    let template_catalog = MemoryWorkspaceCatalog::default();
    let mut template_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000003");
    template_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    template_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Creating)),
        });
    template_catalog
        .insert_workspace(&template_workspace)
        .await
        .expect("insert");
    let template_provider = FakeProvider {
        get_template_error: Some(WorkspaceProvisioningError::ProviderResourceNotFound),
        ..Default::default()
    };
    let template_service = service_with_parts(
        template_catalog,
        template_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let template_result = template_service
        .sync(&template_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &template_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert!(runpod_template_snapshot(&template_result.workspace).is_some());
    assert_eq!(
        template_provider
            .create_template_count
            .load(Ordering::SeqCst),
        0
    );

    let endpoint_catalog = MemoryWorkspaceCatalog::default();
    let mut endpoint_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000004");
    endpoint_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    endpoint_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
        });
    endpoint_workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
        provider_resource_status: ProviderResourceStatus::Creating,
        ..endpoint_snapshot()
    });
    endpoint_catalog
        .insert_workspace(&endpoint_workspace)
        .await
        .expect("insert");
    let endpoint_provider = FakeProvider {
        get_endpoint_error: Some(WorkspaceProvisioningError::ProviderResourceNotFound),
        ..Default::default()
    };
    let endpoint_service = service_with_parts(
        endpoint_catalog,
        endpoint_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let endpoint_result = endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &endpoint_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert!(endpoint_result
        .workspace
        .serverless_endpoint_snapshot
        .is_some());
    assert_eq!(
        endpoint_provider
            .create_endpoint_count
            .load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn cancel_deletes_worker_token_even_without_active_pod_snapshot() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        tokens.clone(),
    );

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
}

#[tokio::test]
async fn cancel_conflict_returns_error_without_cleanup_side_effects() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let coordinator = WorkspaceProvisioningCoordinator::default();
    let _guard = coordinator.try_enter(&workspace.id).expect("enter");
    let service = WorkspaceProvisioningService::new(
        MemorySecretStore {
            api_key: Some("rp_123_secret".to_string()),
            worker_tokens: tokens.clone(),
        },
        provider.clone(),
        catalog.clone(),
        FakeWorker::idle(),
        coordinator,
        test_config(),
    );

    let error = service
        .cancel(&workspace.id)
        .await
        .expect_err("cancel should conflict");

    assert_eq!(error, WorkspaceProvisioningError::ProviderOperationConflict);
    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 0);
    assert!(tokens.lock().expect("tokens").contains_key(&workspace.id));
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.active_provisioning_pod_snapshot.is_some());
    assert!(stored.serverless_endpoint_snapshot.is_some());
}

fn service(
    catalog: MemoryWorkspaceCatalog,
    provider: FakeProvider,
) -> WorkspaceProvisioningService<MemorySecretStore, FakeProvider, MemoryWorkspaceCatalog, FakeWorker>
{
    service_with_parts(catalog, provider, FakeWorker::idle(), Arc::default())
}

fn service_with_parts(
    catalog: MemoryWorkspaceCatalog,
    provider: FakeProvider,
    worker: FakeWorker,
    worker_tokens: Arc<Mutex<HashMap<String, String>>>,
) -> WorkspaceProvisioningService<MemorySecretStore, FakeProvider, MemoryWorkspaceCatalog, FakeWorker>
{
    WorkspaceProvisioningService::new(
        MemorySecretStore {
            api_key: Some("rp_123_secret".to_string()),
            worker_tokens,
        },
        provider,
        catalog,
        worker,
        WorkspaceProvisioningCoordinator::default(),
        test_config(),
    )
}

fn provisioning_workspace_with_ready_volume(id: &str) -> Workspace {
    let mut workspace = sample_workspace(id);
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
        mount_path: "/workspace".to_string(),
    });
    workspace
}

fn active_pod() -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "pod-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Running,
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        provisioner_status_url: "http://203.0.113.10:30001/status".to_string(),
    }
}

fn discovered_pod(provider_resource_id: &str) -> ProvisioningPodObservation {
    ProvisioningPodObservation {
        provider_resource_id: provider_resource_id.to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        provider_resource_status: ProviderResourceStatus::Running,
        provisioner_status_url: Some(format!(
            "https://{provider_resource_id}-8080.proxy.runpod.net/status"
        )),
    }
}

fn discovered_volume(provider_resource_id: &str) -> NetworkVolumeObservation {
    NetworkVolumeObservation {
        provider_resource_id: provider_resource_id.to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provisioned_size_bytes: 80 * 1024 * 1024 * 1024,
        provider_resource_status: ProviderResourceStatus::Ready,
        mount_path: "/workspace".to_string(),
    }
}

fn discovered_template(template_id: &str) -> EndpointTemplateObservation {
    EndpointTemplateObservation {
        template_id: template_id.to_string(),
        endpoint_worker_image_ref: "ghcr.io/luma-forge/endpoint-worker:test".to_string(),
        mount_path: "/workspace".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
    }
}

fn template_snapshot(status: ProviderResourceStatus) -> RunPodEndpointTemplateSnapshot {
    RunPodEndpointTemplateSnapshot {
        template_id: "template-1".to_string(),
        endpoint_worker_image_ref: "ghcr.io/luma-forge/endpoint-worker:test".to_string(),
        mount_path: "/workspace".to_string(),
        provider_resource_status: status,
    }
}

fn discovered_endpoint(provider_resource_id: &str) -> ServerlessEndpointObservation {
    ServerlessEndpointObservation {
        provider_resource_id: provider_resource_id.to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        endpoint_invoke_url: format!("https://api.runpod.ai/v2/{provider_resource_id}/runsync"),
    }
}

fn endpoint_snapshot() -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        endpoint_invoke_url: "https://api.runpod.ai/v2/endpoint-1/runsync".to_string(),
    }
}

fn assert_failure(
    workspace: &Workspace,
    code: WorkspaceProvisioningFailureCode,
    phase: WorkspaceProvisioningPhase,
) {
    assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed);
    let failure = workspace
        .last_provisioning_failure
        .as_ref()
        .expect("failure should be persisted");
    assert_eq!(failure.code, code);
    assert_eq!(failure.phase, phase);
}

fn worker_token_map(workspace_id: &str) -> Arc<Mutex<HashMap<String, String>>> {
    Arc::new(Mutex::new(HashMap::from([(
        workspace_id.to_string(),
        "worker-token".to_string(),
    )])))
}

fn test_config() -> WorkspaceProvisioningConfig {
    WorkspaceProvisioningConfig {
        provisioner_worker_port: 8080,
        runpod_endpoint_worker_port: 8080,
        volume_mount_path: "/workspace".to_string(),
    }
}
