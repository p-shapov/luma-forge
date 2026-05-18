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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_runtime_catalog_is_readable() {
        let catalog = BundledCatalogReader
            .runtime_catalog()
            .expect("bundled runtime catalog should be valid");

        assert!(
            !catalog.contracts.is_empty(),
            "bundled runtime catalog should expose at least one contract"
        );
    }

    #[test]
    fn bundled_workflow_catalog_is_readable_and_runtime_compatible() {
        let reader = BundledCatalogReader;
        let runtime_catalog = reader
            .runtime_catalog()
            .expect("bundled runtime catalog should be valid");
        let workflow_catalog = reader
            .workflow_catalog()
            .expect("bundled workflow catalog should be valid");

        assert!(
            !workflow_catalog.workflow_presets.is_empty(),
            "bundled workflow catalog should expose at least one preset"
        );

        for preset in &workflow_catalog.workflow_presets {
            assert!(
                runtime_catalog
                    .resolve(
                        &preset.runtime_contract.id,
                        &preset.runtime_contract.version
                    )
                    .is_some(),
                "preset {} should reference a bundled runtime contract",
                preset.id
            );
        }
    }

    #[test]
    fn bundled_workflow_catalog_contains_initial_basic_t2i_preset() {
        let catalog = BundledCatalogReader
            .workflow_catalog()
            .expect("bundled workflow catalog should be valid");

        assert!(
            catalog
                .workflow_presets
                .iter()
                .any(|preset| preset.id == "comfyui-t2i-basic"),
            "initial bundled catalog should contain the basic text-to-image preset"
        );
    }
}
