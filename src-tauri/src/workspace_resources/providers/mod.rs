mod runpod;

use std::{future::Future, pin::Pin};

use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    WorkspaceResourceConfig, WorkspaceResourceContext, WorkspaceResourceError,
    WorkspaceResourceSyncResult,
};

pub(crate) trait WorkspaceResourceProvider<S, W>: Send + Sync {
    fn sync_network_volume<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn finish_provisioning_pod<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_serverless_endpoint<'a>(
        &'a self,
        context: &'a WorkspaceResourceContext<'_, S, W>,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

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

#[derive(Debug, Default)]
pub(crate) struct WorkspaceResourceProviderRegistry {
    runpod: runpod::RunPodWorkspaceResourceProvider,
}

impl<S, W> WorkspaceResourceProviderResolver<S, W> for WorkspaceResourceProviderRegistry
where
    S: SecretStore,
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
        domain::provider_setup::ProviderApiKey,
        secrets::{ProvisionerWorkerBearerToken, SecretStoreError},
    };

    #[derive(Debug)]
    struct TestSecretStore;

    impl SecretStore for TestSecretStore {
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

    #[test]
    fn registry_selects_runpod_capability_for_runpod_provider() {
        let registry = WorkspaceResourceProviderRegistry::default();

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
