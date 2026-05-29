use std::{future::Future, pin::Pin};

use crate::{
    domain::workspace::{Workspace, WorkspaceCatalog},
    workspace_setup::error::WorkspaceSetupError,
};

pub trait WorkspaceCatalogRepository: Send + Sync {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>;

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>;

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>;

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableWorkspaceCatalog;

impl WorkspaceCatalogRepository for UnavailableWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn insert_workspace<'a>(
        &'a self,
        _workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        _id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn update_workspace<'a>(
        &'a self,
        _workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }
}
