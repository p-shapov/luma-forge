use crate::domain::workflow_preset::WorkflowCatalog;

use super::WorkflowCatalogError;

pub trait WorkflowCatalogRepository: Send + Sync {
    fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError>;
}
