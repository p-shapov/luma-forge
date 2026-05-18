use crate::{
    domain::workspace::Workspace,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioner::ProvisionerWorkerGateway,
    workspace_provisioner::{WorkspaceProvisionerContext, WorkspaceProvisionerService},
    workspace_resources::{WorkspaceResourceConfig, WorkspaceResourceService},
};

use super::{
    helpers::WorkspaceProvisioningResult, WorkspaceProvisioningConfig, WorkspaceProvisioningError,
};

pub(crate) type SyncStepResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

pub(crate) struct WorkspaceProvisioningContext<'a, S, W, R> {
    pub(crate) secrets: &'a S,
    pub(crate) resources: &'a WorkspaceResourceService<S, W>,
    pub(crate) workspace_catalog: &'a W,
    pub(crate) workers: &'a R,
    pub(crate) workspace_provisioner: &'a WorkspaceProvisionerService,
    pub(crate) config: &'a WorkspaceProvisioningConfig,
}

impl<'a, S, W, R> WorkspaceProvisioningContext<'a, S, W, R> {
    pub(crate) fn new(
        secrets: &'a S,
        resources: &'a WorkspaceResourceService<S, W>,
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

impl<S, W, R> WorkspaceProvisioningContext<'_, S, W, R>
where
    S: SecretStore,
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

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}
