#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace not found")]
    NotFound,
    #[error("workspace already exists")]
    AlreadyExists,
    #[error("workflow not found")]
    WorkflowNotFound,
    #[error("workspace has an attached runtime")]
    RuntimeAttached,
    #[error("workspace has a running operation")]
    OperationRunning,
    #[error("workflow catalog is unavailable")]
    CatalogUnavailable,
    #[error("workspace persistence is unavailable")]
    PersistenceUnavailable,
}
