use crate::{
    bundled_catalog::parser::{parse_runtime_catalog, parse_workflow_catalog},
    domain::{runtime::RuntimeCatalog, workflow::WorkflowCatalog},
    workspace_setup::{error::WorkspaceSetupError, WorkspaceSetupCatalogReader},
};

const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");
const RUNTIME_CATALOG_JSON: &str = include_str!("../../../bundled/runtime-catalog.json");

#[derive(Debug, Clone, Default)]
pub struct BundledCatalogReader;

impl WorkspaceSetupCatalogReader for BundledCatalogReader {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        let runtime_catalog = self.runtime_catalog()?;
        parse_workflow_catalog(WORKFLOW_CATALOG_JSON, &runtime_catalog)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }

    fn runtime_catalog(&self) -> Result<RuntimeCatalog, WorkspaceSetupError> {
        parse_runtime_catalog(RUNTIME_CATALOG_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }
}
