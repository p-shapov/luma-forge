mod runpod;

use std::{future::Future, pin::Pin};

use crate::{
    domain::{provider_setup::GpuCloudProviderId, workspace::Workspace},
    secrets::AsyncSecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    context::{SyncStepResult, WorkspaceProvisioningContext, WorkspaceProvisioningResources},
    gateway::ProvisionerWorkerGateway,
    WorkspaceProvisioningError,
};

pub(crate) trait WorkspaceProvisioningProvider<S, W, R, Q>: Send + Sync {
    fn sync<'a>(
        &'a self,
        context: &'a WorkspaceProvisioningContext<'_, S, W, R, Q>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = SyncStepResult> + Send + 'a>>;

    fn cancel<'a>(
        &'a self,
        context: &'a WorkspaceProvisioningContext<'_, S, W, R, Q>,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceProvisioningError>> + Send + 'a>>;
}

pub(crate) trait WorkspaceProvisioningProviderResolver<S, W, R, Q>: Send + Sync {
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceProvisioningProvider<S, W, R, Q>;
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceProvisioningProviderRegistry {
    runpod: runpod::RunPodWorkspaceProvisioningProvider,
}

impl<S, W, R, Q> WorkspaceProvisioningProviderResolver<S, W, R, Q>
    for WorkspaceProvisioningProviderRegistry
where
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
    Q: WorkspaceProvisioningResources,
{
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceProvisioningProvider<S, W, R, Q> {
        match provider_id {
            GpuCloudProviderId::Runpod => &self.runpod,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_provisioning::test_support::{
        FakeProvisionerWorkerGateway, FakeSecretStore, FakeWorkspaceCatalog, FakeWorkspaceResources,
    };

    #[test]
    fn registry_selects_runpod_capability_for_runpod_provider() {
        let registry = WorkspaceProvisioningProviderRegistry::default();

        let capability =
            <WorkspaceProvisioningProviderRegistry as WorkspaceProvisioningProviderResolver<
                FakeSecretStore,
                FakeWorkspaceCatalog,
                FakeProvisionerWorkerGateway,
                FakeWorkspaceResources,
            >>::for_provider(&registry, &GpuCloudProviderId::Runpod);

        assert!(std::ptr::addr_eq(
            capability
                as *const dyn WorkspaceProvisioningProvider<
                    FakeSecretStore,
                    FakeWorkspaceCatalog,
                    FakeProvisionerWorkerGateway,
                    FakeWorkspaceResources,
                >,
            &registry.runpod
                as &dyn WorkspaceProvisioningProvider<
                    FakeSecretStore,
                    FakeWorkspaceCatalog,
                    FakeProvisionerWorkerGateway,
                    FakeWorkspaceResources,
                >
                as *const dyn WorkspaceProvisioningProvider<
                    FakeSecretStore,
                    FakeWorkspaceCatalog,
                    FakeProvisionerWorkerGateway,
                    FakeWorkspaceResources,
                >,
        ));
    }
}
