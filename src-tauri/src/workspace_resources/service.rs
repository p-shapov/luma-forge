use crate::{
    domain::workspace::Workspace,
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::state::reset_after_resource_cleanup,
};

use super::{
    WorkspaceResourceContext, WorkspaceResourceError, WorkspaceResourceProviderRegistry,
    WorkspaceResourceProviderResolver,
};

pub(crate) type WorkspaceResourceOperationResult =
    Result<Option<Workspace>, WorkspaceResourceError>;

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceResourceService<S, W, R = WorkspaceResourceProviderRegistry> {
    secrets: S,
    workspace_catalog: W,
    provider_registry: R,
}

impl<S, W, R> WorkspaceResourceService<S, W, R> {
    pub(crate) fn with_provider_registry(
        secrets: S,
        workspace_catalog: W,
        provider_registry: R,
    ) -> Self {
        Self {
            secrets,
            workspace_catalog,
            provider_registry,
        }
    }

    fn context(&self) -> WorkspaceResourceContext<'_, S, W> {
        WorkspaceResourceContext::new(&self.secrets, &self.workspace_catalog)
    }
}

impl<S, W, R> WorkspaceResourceService<S, W, R>
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    R: WorkspaceResourceProviderResolver<S, W>,
{
    pub(crate) async fn create_network_volume(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .create_network_volume(&context, workspace)
            .await
    }

    pub(crate) async fn observe_network_volume(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .observe_network_volume(&context, workspace)
            .await
    }

    pub(crate) async fn create_provisioning_pod(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .create_provisioning_pod(&context, workspace)
            .await
    }

    pub(crate) async fn observe_provisioning_pod(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .observe_provisioning_pod(&context, workspace)
            .await
    }

    pub(crate) async fn delete_provisioning_pod(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .delete_provisioning_pod(&context, workspace)
            .await
    }

    pub(crate) async fn create_serverless_endpoint(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .create_serverless_endpoint(&context, workspace)
            .await
    }

    pub(crate) async fn observe_serverless_endpoint(
        &self,
        workspace: &mut Workspace,
    ) -> WorkspaceResourceOperationResult {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .observe_serverless_endpoint(&context, workspace)
            .await
    }

    pub(crate) async fn cleanup_known_resources(
        &self,
        workspace: &mut Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        let context = self.context();
        self.provider_registry
            .for_provider(&workspace.gpu_cloud_provider_id)
            .cleanup_known_resources(&context, workspace)
            .await?;
        reset_after_resource_cleanup(workspace);
        context.update_workspace(workspace).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            hugging_face_setup::HuggingFaceApiKey,
            placement::PlacementPlan,
            provider_setup::{GpuCloudProviderId, ProviderApiKey},
            provisioner::ResolvedProvisionerImageSnapshot,
            runtime::ResolvedRuntimeImageSnapshot,
            workflow::{
                ProvisionerContractReference, RuntimeContractReference, WorkflowExecutionType,
                WorkflowPreset,
            },
            workspace::{
                PersistentStorageVolumeSnapshot, ProviderResourceStatus, WorkspaceCatalog,
                WorkspaceLifecycleState,
            },
        },
        secrets::{
            HuggingFaceApiKeyStore, ProviderKeyStore, ProvisionerTokenStore,
            ProvisionerWorkerBearerToken, SecretStoreError,
        },
        workspace_catalog::repository::WorkspaceCatalogRepository,
        workspace_resources::providers::{
            WorkspaceResourceProvider, WorkspaceResourceProviderResolver,
        },
        workspace_setup::error::WorkspaceSetupError,
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ResourceCall {
        CreateNetworkVolume,
        ObserveNetworkVolume,
        CreateProvisioningPod,
        ObserveProvisioningPod,
        DeleteProvisioningPod,
        CreateServerlessEndpoint,
        ObserveServerlessEndpoint,
        CleanupKnownResources,
    }

    #[derive(Debug, Clone, Default)]
    struct FakeResourceProvider {
        calls: Arc<Mutex<Vec<ResourceCall>>>,
    }

    impl FakeResourceProvider {
        fn calls(&self) -> Vec<ResourceCall> {
            self.calls.lock().expect("fake provider calls").clone()
        }

        fn record(&self, call: ResourceCall) {
            self.calls.lock().expect("fake provider calls").push(call);
        }
    }

    impl WorkspaceResourceProvider<FakeSecretStore, FakeWorkspaceCatalog> for FakeResourceProvider {
        fn create_network_volume<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::CreateNetworkVolume);
            Box::pin(async { Ok(None) })
        }

        fn observe_network_volume<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::ObserveNetworkVolume);
            Box::pin(async { Ok(None) })
        }

        fn create_provisioning_pod<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::CreateProvisioningPod);
            Box::pin(async { Ok(None) })
        }

        fn observe_provisioning_pod<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::ObserveProvisioningPod);
            Box::pin(async { Ok(None) })
        }

        fn delete_provisioning_pod<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::DeleteProvisioningPod);
            Box::pin(async { Ok(None) })
        }

        fn create_serverless_endpoint<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::CreateServerlessEndpoint);
            Box::pin(async { Ok(None) })
        }

        fn observe_serverless_endpoint<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a mut Workspace,
        ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
            self.record(ResourceCall::ObserveServerlessEndpoint);
            Box::pin(async { Ok(None) })
        }

        fn cleanup_known_resources<'a>(
            &'a self,
            _context: &'a WorkspaceResourceContext<'_, FakeSecretStore, FakeWorkspaceCatalog>,
            _workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>> {
            self.record(ResourceCall::CleanupKnownResources);
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Debug, Clone)]
    struct FakeProviderResolver {
        provider: FakeResourceProvider,
    }

    impl WorkspaceResourceProviderResolver<FakeSecretStore, FakeWorkspaceCatalog>
        for FakeProviderResolver
    {
        fn for_provider(
            &self,
            provider_id: &GpuCloudProviderId,
        ) -> &dyn WorkspaceResourceProvider<FakeSecretStore, FakeWorkspaceCatalog> {
            assert_eq!(*provider_id, GpuCloudProviderId::Runpod);
            &self.provider
        }
    }

    #[derive(Debug, Clone, Default)]
    struct FakeSecretStore;

    impl ProviderKeyStore for FakeSecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            Ok(false)
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            Ok(None)
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            _api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
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
            _workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            Ok(None)
        }

        fn delete_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }
    }

    impl HuggingFaceApiKeyStore for FakeSecretStore {
        fn has_hugging_face_api_key_entry(&self) -> Result<bool, SecretStoreError> {
            Ok(false)
        }

        fn read_hugging_face_api_key(&self) -> Result<Option<HuggingFaceApiKey>, SecretStoreError> {
            Ok(None)
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

    #[derive(Debug, Clone, Default)]
    struct FakeWorkspaceCatalog {
        updates: Arc<Mutex<Vec<Workspace>>>,
    }

    impl FakeWorkspaceCatalog {
        fn updates(&self) -> Vec<Workspace> {
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
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
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
        ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceSetupError>> + Send + 'a>> {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }
    }

    #[tokio::test]
    async fn service_delegates_resource_steps_to_selected_provider() {
        let provider = FakeResourceProvider::default();
        let service = service(provider.clone());
        let mut workspace = workspace();

        service
            .create_network_volume(&mut workspace)
            .await
            .expect("network volume create should delegate");
        service
            .observe_network_volume(&mut workspace)
            .await
            .expect("network volume observe should delegate");
        service
            .create_provisioning_pod(&mut workspace)
            .await
            .expect("provisioning pod create should delegate");
        service
            .observe_provisioning_pod(&mut workspace)
            .await
            .expect("provisioning pod observe should delegate");
        service
            .delete_provisioning_pod(&mut workspace)
            .await
            .expect("provisioning pod delete should delegate");
        service
            .create_serverless_endpoint(&mut workspace)
            .await
            .expect("endpoint create should delegate");
        service
            .observe_serverless_endpoint(&mut workspace)
            .await
            .expect("endpoint observe should delegate");

        assert_eq!(
            provider.calls(),
            vec![
                ResourceCall::CreateNetworkVolume,
                ResourceCall::ObserveNetworkVolume,
                ResourceCall::CreateProvisioningPod,
                ResourceCall::ObserveProvisioningPod,
                ResourceCall::DeleteProvisioningPod,
                ResourceCall::CreateServerlessEndpoint,
                ResourceCall::ObserveServerlessEndpoint,
            ]
        );
    }

    #[tokio::test]
    async fn cleanup_delegates_then_clears_resource_snapshots_and_persists_workspace() {
        let provider = FakeResourceProvider::default();
        let catalog = FakeWorkspaceCatalog::default();
        let service = WorkspaceResourceService::with_provider_registry(
            FakeSecretStore,
            catalog.clone(),
            FakeProviderResolver {
                provider: provider.clone(),
            },
        );
        let mut workspace = workspace();
        workspace.lifecycle_state = WorkspaceLifecycleState::Failed;
        workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
        });

        let updated = service
            .cleanup_known_resources(&mut workspace)
            .await
            .expect("cleanup should delegate and persist reset workspace");

        assert_eq!(provider.calls(), vec![ResourceCall::CleanupKnownResources]);
        assert_eq!(updated.lifecycle_state, WorkspaceLifecycleState::Failed);
        assert!(updated.persistent_storage_volume_snapshot.is_none());
        assert_eq!(catalog.updates(), vec![updated]);
    }

    fn service(
        provider: FakeResourceProvider,
    ) -> WorkspaceResourceService<FakeSecretStore, FakeWorkspaceCatalog, FakeProviderResolver> {
        WorkspaceResourceService::with_provider_registry(
            FakeSecretStore,
            FakeWorkspaceCatalog::default(),
            FakeProviderResolver { provider },
        )
    }

    fn workspace() -> Workspace {
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
}
