use crate::domain::workflow_preset::WorkflowCatalog;

use super::{
    errors::WorkflowCatalogError,
    execution_schemas::{
        read_bundled_execution_schema_registry, validate_execution_schema_registry,
    },
    reader::read_bundled_workflow_catalog,
    validation::validate_workflows,
};

#[derive(Debug, Clone, Default)]
pub struct WorkflowCatalogService;

impl WorkflowCatalogService {
    pub fn new() -> Self {
        Self
    }

    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
        let execution_schemas = read_bundled_execution_schema_registry()?;
        validate_execution_schema_registry(&execution_schemas)?;

        let catalog = read_bundled_workflow_catalog()?;
        validate_workflows(&catalog.workflow_presets, &execution_schemas)?;

        Ok(catalog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> WorkflowCatalogService {
        WorkflowCatalogService::new()
    }

    #[test]
    fn get_workflow_catalog_returns_valid_workflows() {
        let workflows = service()
            .get_workflow_catalog()
            .expect("workflows should be valid");

        assert!(
            workflows
                .workflow_presets
                .iter()
                .any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }
}
