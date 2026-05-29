mod runpod;

use std::{future::Future, pin::Pin};

use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{WorkspaceResourceContext, WorkspaceResourceError, WorkspaceResourceOperationResult};

pub(crate) trait WorkspaceResourceProvider<S, W>: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn delete_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn cleanup_known_resources<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceResourceError>> + Send + 'a>>;
}

pub(crate) trait WorkspaceResourceProviderResolver<S, W>: Send + Sync {
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceResourceProvider<S, W>;
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceResourceProviderRegistry {
    runpod: runpod::RunPodWorkspaceResourceProvider,
}

impl WorkspaceResourceProviderRegistry {
    pub(crate) fn try_new() -> Result<Self, crate::provider::runpod::RunPodHttpClientInitError> {
        Ok(Self {
            runpod: runpod::RunPodWorkspaceResourceProvider::try_new()?,
        })
    }
}

impl<S, W> WorkspaceResourceProviderResolver<S, W> for WorkspaceResourceProviderRegistry
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
{
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceResourceProvider<S, W> {
        match provider_id {
            GpuCloudProviderId::Runpod => &self.runpod,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::hugging_face_setup::HuggingFaceApiKey,
        domain::provider_setup::ProviderApiKey,
        secrets::{
            HuggingFaceApiKeyStore, ProviderKeyStore, ProvisionerTokenStore,
            ProvisionerWorkerBearerToken, SecretStoreError,
        },
    };

    #[derive(Debug)]
    struct TestSecretStore;

    impl ProviderKeyStore for TestSecretStore {
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

    impl ProvisionerTokenStore for TestSecretStore {
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

    impl HuggingFaceApiKeyStore for TestSecretStore {
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

    #[test]
    fn registry_selects_runpod_capability_for_runpod_provider() {
        let registry = WorkspaceResourceProviderRegistry::try_new().expect("registry initializes");

        let capability = <WorkspaceResourceProviderRegistry as WorkspaceResourceProviderResolver<
            TestSecretStore,
            crate::workspace_catalog::repository::UnavailableWorkspaceCatalog,
        >>::for_provider(&registry, &GpuCloudProviderId::Runpod);

        assert!(std::ptr::addr_eq(
            capability
                as *const dyn WorkspaceResourceProvider<
                    TestSecretStore,
                    crate::workspace_catalog::repository::UnavailableWorkspaceCatalog,
                >,
            &registry.runpod
                as &dyn WorkspaceResourceProvider<
                    TestSecretStore,
                    crate::workspace_catalog::repository::UnavailableWorkspaceCatalog,
                >
                as *const dyn WorkspaceResourceProvider<
                    TestSecretStore,
                    crate::workspace_catalog::repository::UnavailableWorkspaceCatalog,
                >,
        ));
    }
}
