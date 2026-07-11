use crate::application::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceRepositoryError {
    #[error("workspace already exists")]
    AlreadyExists,
    #[error("workspace persistence is unavailable")]
    Unavailable,
    #[error("workspace persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn create(&self, workspace: Workspace) -> Result<Workspace, WorkspaceRepositoryError>;
    async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError>;
    async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError>;
    async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError>;
}
