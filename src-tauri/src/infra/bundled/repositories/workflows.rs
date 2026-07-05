use super::super::catalog::{BundledCatalog, WorkflowRevisionPaths};
use super::super::errors::BundledCatalogError;

#[derive(Debug, Clone, Default)]
pub struct BundledWorkflowRepository {
    catalog: BundledCatalog,
}

impl BundledWorkflowRepository {
    pub fn new() -> Self {
        Self {
            catalog: BundledCatalog::new(),
        }
    }

    #[cfg(test)]
    pub fn from_catalog(catalog: BundledCatalog) -> Self {
        Self { catalog }
    }

    pub fn workflow_revision_count(&self) -> usize {
        self.catalog.workflow_revision_paths().len()
    }

    pub fn list(&self) -> Result<Vec<WorkflowRevisionPaths>, BundledCatalogError> {
        Ok(self
            .catalog
            .workflow_revision_paths()
            .into_values()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE_ASSETS: &[(&str, &str)] = &[
        ("workflows/example/1.0.0/metadata.json", "{}"),
        ("workflows/example/1.0.0/model_assets.json", "{}"),
        ("workflows/example/1.0.0/contract_requirements.json", "{}"),
        ("workflows/example/1.0.0/execution_contract.json", "{}"),
        ("workflows/example/1.0.0/workflow.json", "{}"),
    ];

    #[test]
    fn list_returns_grouped_workflow_revisions() {
        let repository =
            BundledWorkflowRepository::from_catalog(BundledCatalog::from_assets(FIXTURE_ASSETS));
        let revisions = repository.list().expect("workflows should parse");

        assert_eq!(repository.workflow_revision_count(), 1);
        assert_eq!(revisions.len(), 1);
    }
}
