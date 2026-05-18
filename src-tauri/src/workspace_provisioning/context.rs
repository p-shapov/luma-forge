use crate::{
    domain::workspace::Workspace,
    provisioner_worker::ProvisionerWorkerGateway,
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::operations::{WorkspaceResourceConfig, WorkspaceResourceOperations},
};

use super::{
    helpers::WorkspaceProvisioningResult, WorkspaceProvisioningConfig, WorkspaceProvisioningError,
};

pub(crate) type SyncStepResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

pub(crate) struct WorkspaceProvisioningContext<'a, S, Q, W, R> {
    pub(crate) secrets: &'a S,
    pub(crate) resources: &'a Q,
    pub(crate) workspace_catalog: &'a W,
    pub(crate) workers: &'a R,
    pub(crate) config: &'a WorkspaceProvisioningConfig,
}

impl<'a, S, Q, W, R> WorkspaceProvisioningContext<'a, S, Q, W, R> {
    pub(crate) fn new(
        secrets: &'a S,
        resources: &'a Q,
        workspace_catalog: &'a W,
        workers: &'a R,
        config: &'a WorkspaceProvisioningConfig,
    ) -> Self {
        Self {
            secrets,
            resources,
            workspace_catalog,
            workers,
            config,
        }
    }

    pub(crate) fn resource_config(&self) -> WorkspaceResourceConfig {
        WorkspaceResourceConfig {
            volume_mount_path: self.config.volume_mount_path.clone(),
        }
    }
}

impl<S, Q, W, R> WorkspaceProvisioningContext<'_, S, Q, W, R>
where
    S: SecretStore,
    Q: WorkspaceResourceOperations,
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
