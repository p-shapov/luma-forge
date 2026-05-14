use crate::{
    bundled_catalog::parser::parse_workflow_catalog,
    domain::workflow::WorkflowCatalog,
    workspace_setup::{error::WorkspaceSetupError, WorkspaceSetupCatalogReader},
};

const WORKFLOW_CATALOG_JSON: &str =
    include_str!("../../../resources/catalog/workflow-catalog.json");

#[derive(Debug, Clone, Default)]
pub struct BundledCatalogReader;

impl WorkspaceSetupCatalogReader for BundledCatalogReader {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        parse_workflow_catalog(WORKFLOW_CATALOG_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }
}
