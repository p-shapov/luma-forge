use crate::application::workspace::Workspace;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
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
    async fn page(
        &self,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Workspace>, u64), WorkspaceRepositoryError>;
    async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError>;
}
