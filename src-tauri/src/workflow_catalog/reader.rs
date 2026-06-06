use crate::domain::{runtime_contract::RuntimeCatalog, workflow_preset::WorkflowCatalog};

use super::WorkflowCatalogError;

const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");
const ENDPOINT_CONTRACTS_JSON: &str = include_str!("../../../bundled/endpoint-contracts.json");
const PROVISIONER_CONTRACTS_JSON: &str =
    include_str!("../../../bundled/provisioner-contracts.json");

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledWorkflowCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledEndpointContractCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledProvisionerContractCatalogReader;

impl BundledWorkflowCatalogReader {
    pub fn read_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
        serde_json::from_str(WORKFLOW_CATALOG_JSON).map_err(|_| WorkflowCatalogError::ParseFailed)
    }
}

impl BundledEndpointContractCatalogReader {
    pub fn read_endpoint_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        serde_json::from_str(ENDPOINT_CONTRACTS_JSON).map_err(|_| WorkflowCatalogError::ParseFailed)
    }
}

impl BundledProvisionerContractCatalogReader {
    pub fn read_provisioner_contract_catalog(
        &self,
    ) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        serde_json::from_str(PROVISIONER_CONTRACTS_JSON)
            .map_err(|_| WorkflowCatalogError::ParseFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
        BundledWorkflowCatalogReader,
    };

    #[test]
    fn bundled_workflow_reader_deserializes_workflows() {
        let workflows = BundledWorkflowCatalogReader
            .read_workflow_catalog()
            .expect("bundled workflows should deserialize");

        assert!(
            workflows
                .workflow_presets
                .iter()
                .any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }

    #[test]
    fn bundled_endpoint_contract_reader_deserializes_contracts() {
        let catalog = BundledEndpointContractCatalogReader
            .read_endpoint_contract_catalog()
            .expect("bundled endpoint contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream endpoint contract"
        );
    }

    #[test]
    fn bundled_provisioner_contract_reader_deserializes_contracts() {
        let catalog = BundledProvisionerContractCatalogReader
            .read_provisioner_contract_catalog()
            .expect("bundled provisioner contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "luma-forge-provisioner"),
            "expected bundled provisioner contract"
        );
    }
}
