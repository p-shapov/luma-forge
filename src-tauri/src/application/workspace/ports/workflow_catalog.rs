use async_trait::async_trait;

use crate::application::runtimes::{WorkflowDefinition, WorkflowSummary};

#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowCatalogError {
    #[error("bundled catalog is invalid")]
    InvalidCatalog,
    #[error("bundled catalog is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait WorkflowCatalog: Send + Sync {
    async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError>;
    async fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError>;
}
