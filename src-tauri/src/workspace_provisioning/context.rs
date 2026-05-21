use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::Workspace,
    secrets::AsyncSecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        WorkspaceResourceConfig, WorkspaceResourceError, WorkspaceResourceService,
        WorkspaceResourceSyncResult,
    },
};

use super::{
    gateway::ProvisionerWorkerGateway,
    helpers::{catalog_error, WorkspaceProvisioningResult},
    provisioner::{WorkspaceProvisionerContext, WorkspaceProvisionerService},
    WorkspaceProvisioningConfig, WorkspaceProvisioningError,
};

pub(crate) type SyncStepResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

pub(crate) trait WorkspaceProvisioningResources: Send + Sync {
    fn sync_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn finish_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn sync_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>>;

    fn cleanup_known_resources<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceResourceError>> + Send + 'a>>;
}

impl<S, W> WorkspaceProvisioningResources for WorkspaceResourceService<S, W>
where
    S: AsyncSecretStore,
    W: WorkspaceCatalogRepository,
{
    fn sync_network_volume<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::sync_network_volume(self, workspace, config).await
        })
    }

    fn sync_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::sync_provisioning_pod(self, workspace, config).await
        })
    }

    fn finish_provisioning_pod<'a>(
        &'a self,
        workspace: &'a mut Workspace,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(
            async move { WorkspaceResourceService::finish_provisioning_pod(self, workspace).await },
        )
    }

    fn sync_serverless_endpoint<'a>(
        &'a self,
        workspace: &'a mut Workspace,
        config: &'a WorkspaceResourceConfig,
    ) -> Pin<Box<dyn Future<Output = WorkspaceResourceSyncResult> + Send + 'a>> {
        Box::pin(async move {
            WorkspaceResourceService::sync_serverless_endpoint(self, workspace, config).await
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
    pub(crate) config: &'a WorkspaceProvisioningConfig,
}

impl<'a, S, W, R, Q> WorkspaceProvisioningContext<'a, S, W, R, Q> {
    pub(crate) fn new(
        secrets: &'a S,
        resources: &'a Q,
        workspace_catalog: &'a W,
        workers: &'a R,
        workspace_provisioner: &'a WorkspaceProvisionerService,
        config: &'a WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            workspace_provisioner,
            config,
        }
    }

    pub(crate) fn resource_config(&self) -> WorkspaceResourceConfig {
        WorkspaceResourceConfig {
            volume_mount_path: self.config.volume_mount_path.clone(),
        }
    }

    pub(crate) fn workspace_provisioner_context(&self) -> WorkspaceProvisionerContext<'_, S, W, R> {
        WorkspaceProvisionerContext::new(self.secrets, self.workspace_catalog, self.workers)
    }
}

impl<S, W, R, Q> WorkspaceProvisioningContext<'_, S, W, R, Q>
where
    S: AsyncSecretStore,
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
