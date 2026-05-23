use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::Workspace,
    secrets::{AsyncHuggingFaceApiKeyStore, AsyncProviderKeyStore, AsyncProvisionerTokenStore},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        WorkspaceResourceError, WorkspaceResourceOperationResult, WorkspaceResourceService,
    },
};

use super::{
    gateway::ProvisionerWorkerGateway,
    helpers::{catalog_error, WorkspaceProvisioningResult},
    provisioner::{WorkspaceProvisionerContext, WorkspaceProvisionerService},
    WorkspaceProvisioningError,
};

pub(crate) type SyncStepResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

pub(crate) trait WorkspaceProvisioningResources: Send + Sync {
    fn create_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn create_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn delete_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn observe_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>>;

    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>>;
}

impl<S, W> WorkspaceProvisioningResources for WorkspaceResourceService<S, W>
where
    S: AsyncHuggingFaceApiKeyStore + AsyncProviderKeyStore + AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
{
    fn create_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::create_network_volume(self, workspace).await },
        )
    }

    fn observe_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::observe_network_volume(self, workspace).await },
        )
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::create_provisioning_pod(self, workspace).await },
        )
    }

    fn observe_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::observe_provisioning_pod(self, workspace).await
        })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::delete_provisioning_pod(self, workspace).await },
        )
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::create_serverless_endpoint(self, workspace).await
        })
    }

    fn observe_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceOperationResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::observe_serverless_endpoint(self, workspace).await
        })
    }

    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::cleanup_known_resources(self, workspace).await },
        )
    }
}

pub(crate) struct WorkspaceProvisioningContext<'a, S, W, R, Q = WorkspaceResourceService<S, W>> {
    pub(crate) secrets: &'a S,
    pub(crate) resources: &'a Q,
    pub(crate) workspace_catalog: &'a W,
    pub(crate) workers: &'a R,
    pub(crate) workspace_provisioner: &'a WorkspaceProvisionerService,
}

impl<'a, S, W, R, Q> WorkspaceProvisioningContext<'a, S, W, R, Q> {
    pub(crate) fn new(
        secrets: &'a S,
        resources: &'a Q,
        workspace_catalog: &'a W,
        workers: &'a R,
        workspace_provisioner: &'a WorkspaceProvisionerService,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            workspace_provisioner,
        }
    }

    pub(crate) fn workspace_provisioner_context(&self) -> WorkspaceProvisionerContext<'_, S, W, R> {
        WorkspaceProvisionerContext::new(self.secrets, self.workspace_catalog, self.workers)
    }
}

impl<S, W, R, Q> WorkspaceProvisioningContext<'_, S, W, R, Q>
where
    S: AsyncProvisionerTokenStore,
    W: WorkspaceCatalogRepository,
    R: ProvisionerWorkerGateway,
{
    pub(crate) async fn workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(catalog_error)?
            .ok_or(WorkspaceProvisioningError::WorkspaceNotFound)
    }

    pub(crate) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(catalog_error)
    }
}
