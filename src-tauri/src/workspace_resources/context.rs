use crate::{
    domain::workspace::Workspace, workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::WorkspaceResourceError;

pub(crate) struct WorkspaceResourceContext<'a, S, W> {
    pub(crate) secrets: &'a S,
    workspace_catalog: &'a W,
}

impl<'a, S, W> WorkspaceResourceContext<'a, S, W> {
    pub(crate) fn new(secrets: &'a S, workspace_catalog: &'a W) -> Self {
        Self {
            secrets,
            workspace_catalog,
        }
    }
}

impl<S, W> WorkspaceResourceContext<'_, S, W>
where
    W: WorkspaceCatalogRepository,
{
    pub(crate) async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceResourceError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(WorkspaceResourceError::from)
    }
}
