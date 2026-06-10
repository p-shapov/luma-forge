use crate::domain::{runtime_contract::RuntimeCatalog, workflow_preset::WorkflowCatalog};

use super::{
    errors::WorkflowCatalogError,
    reader::{
        BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
        BundledWorkflowCatalogReader,
    },
    validation::{validate_runtime_catalog, validate_workflows},
};

#[derive(Debug, Clone, Default)]
pub struct WorkflowCatalogService {
    workflow_reader: BundledWorkflowCatalogReader,
    endpoint_contract_reader: BundledEndpointContractCatalogReader,
    provisioner_contract_reader: BundledProvisionerContractCatalogReader,
}

impl WorkflowCatalogService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
        let catalog = self.workflow_reader.read_workflow_catalog()?;
        let endpoint_contract_catalog = self.get_endpoint_contract_catalog()?;
        let provisioner_contract_catalog = self.get_provisioner_contract_catalog()?;

        validate_workflows(
            &catalog.workflow_presets,
            &endpoint_contract_catalog,
            &provisioner_contract_catalog,
        )?;

        Ok(catalog)
    }

    pub fn get_endpoint_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        let catalog = self
            .endpoint_contract_reader
            .read_endpoint_contract_catalog()?;

        validate_runtime_catalog(&catalog)?;

        Ok(catalog)
    }

    pub fn get_provisioner_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        let catalog = self
            .provisioner_contract_reader
            .read_provisioner_contract_catalog()?;

        validate_runtime_catalog(&catalog)?;

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

    #[test]
    fn get_endpoint_contract_catalog_returns_valid_catalog() {
        let catalog = service()
            .get_endpoint_contract_catalog()
            .expect("endpoint contract catalog should be valid");

        assert!(catalog
            .contracts
            .iter()
            .any(|contract| contract.id == "comfyui-hidream-o1-dev"));
    }

    #[test]
    fn get_provisioner_contract_catalog_returns_valid_catalog() {
        let catalog = service()
            .get_provisioner_contract_catalog()
            .expect("provisioner contract catalog should be valid");

        assert!(catalog
            .contracts
            .iter()
            .any(|contract| contract.id == "luma-forge-provisioner"));
    }
}
