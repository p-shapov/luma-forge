use crate::{
    bundled_catalog::parser::{
        parse_provisioner_catalog, parse_runtime_catalog, parse_workflow_catalog,
    },
    domain::{provisioner::ProvisionerCatalog, runtime::RuntimeCatalog, workflow::WorkflowCatalog},
    workspace_setup::{error::WorkspaceSetupError, WorkspaceSetupCatalogReader},
};

const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");
const RUNTIME_CATALOG_JSON: &str = include_str!("../../../bundled/runtime-catalog.json");
const PROVISIONER_CATALOG_JSON: &str = include_str!("../../../bundled/provisioner-catalog.json");

#[derive(Debug, Clone, Default)]
pub struct BundledCatalogReader;

impl WorkspaceSetupCatalogReader for BundledCatalogReader {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        let runtime_catalog = self.runtime_catalog()?;
        let provisioner_catalog = self.provisioner_catalog()?;
        parse_workflow_catalog(
            WORKFLOW_CATALOG_JSON,
            &runtime_catalog,
            &provisioner_catalog,
        )
        .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }

    fn runtime_catalog(&self) -> Result<RuntimeCatalog, WorkspaceSetupError> {
        parse_runtime_catalog(RUNTIME_CATALOG_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }

    fn provisioner_catalog(&self) -> Result<ProvisionerCatalog, WorkspaceSetupError> {
        parse_provisioner_catalog(PROVISIONER_CATALOG_JSON)
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
    fn bundled_provisioner_catalog_is_readable() {
        let catalog = BundledCatalogReader
            .provisioner_catalog()
            .expect("bundled provisioner catalog should be valid");

        assert!(
            !catalog.contracts.is_empty(),
            "bundled provisioner catalog should expose at least one contract"
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
        let provisioner_catalog = reader
            .provisioner_catalog()
            .expect("bundled provisioner catalog should be valid");

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
            assert!(
                provisioner_catalog
                    .resolve(
                        &preset.provisioner_contract.id,
                        &preset.provisioner_contract.version
                    )
                    .is_some(),
                "preset {} should reference a bundled provisioner contract",
                preset.id
            );
        }
    }

    #[test]
    fn bundled_workflow_catalog_presets_include_model_assets() {
        let catalog = BundledCatalogReader
            .workflow_catalog()
            .expect("bundled workflow catalog should be valid");

        for preset in &catalog.workflow_presets {
            assert!(
                !preset.required_model_assets.is_empty(),
                "preset {} should declare at least one required model asset",
                preset.id
            );
        }
    }
}
