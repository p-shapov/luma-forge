use crate::domain::workspace::{Workspace, WorkspaceCatalog};

use super::errors::WorkspaceCatalogError;

#[async_trait::async_trait]
pub trait WorkspaceCatalogRepository: Send + Sync {
    async fn list_workspaces(&self) -> Result<WorkspaceCatalog, WorkspaceCatalogError>;

    async fn find_workspace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<Workspace>, WorkspaceCatalogError>;

    async fn insert_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError>;

    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError>;

    async fn delete_workspace(&self, id: &str) -> Result<(), WorkspaceCatalogError>;
}
