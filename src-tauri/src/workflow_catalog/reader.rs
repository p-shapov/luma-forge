use serde::Deserialize;
use serde_json::Value;

use crate::domain::{
    runtime_contract::RuntimeCatalog,
    workflow_preset::{
        ModelAsset, WorkflowCatalog, WorkflowExecutionType, WorkflowPreset, WorkflowRevision,
    },
};

use super::{contract_requirements::decode_contract_requirements, WorkflowCatalogError};

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
        let document: WorkflowCatalogDocument =
            serde_json::from_str(WORKFLOW_CATALOG_JSON).map_err(parse_error)?;

        document.decode()
    }
}

impl BundledEndpointContractCatalogReader {
    pub fn read_endpoint_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        serde_json::from_str(ENDPOINT_CONTRACTS_JSON).map_err(parse_error)
    }
}

impl BundledProvisionerContractCatalogReader {
    pub fn read_provisioner_contract_catalog(
        &self,
    ) -> Result<RuntimeCatalog, WorkflowCatalogError> {
        serde_json::from_str(PROVISIONER_CONTRACTS_JSON).map_err(parse_error)
    }
}

fn parse_error(error: serde_json::Error) -> WorkflowCatalogError {
    WorkflowCatalogError::ParseFailed {
        message: error.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowCatalogDocument {
    workflow_presets: Vec<WorkflowPresetDocument>,
}

#[derive(Debug, Deserialize)]
struct WorkflowPresetDocument {
    id: String,
    name: String,
    execution_type: WorkflowExecutionType,
    revisions: Vec<WorkflowRevisionDocument>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRevisionDocument {
    version: String,
    requires_hugging_face_api_key: bool,
    required_volume_size_gb: u64,
    contract_requirements: Vec<Value>,
    required_model_assets: Vec<ModelAsset>,
}

impl WorkflowCatalogDocument {
    fn decode(self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
        Ok(WorkflowCatalog {
            workflow_presets: self
                .workflow_presets
                .into_iter()
                .map(WorkflowPresetDocument::decode)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl WorkflowPresetDocument {
    fn decode(self) -> Result<WorkflowPreset, WorkflowCatalogError> {
        Ok(WorkflowPreset {
            id: self.id,
            name: self.name,
            execution_type: self.execution_type,
            revisions: self
                .revisions
                .into_iter()
                .map(WorkflowRevisionDocument::decode)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl WorkflowRevisionDocument {
    fn decode(self) -> Result<WorkflowRevision, WorkflowCatalogError> {
        Ok(WorkflowRevision {
            version: self.version,
            requires_hugging_face_api_key: self.requires_hugging_face_api_key,
            required_volume_size_gb: self.required_volume_size_gb,
            contract_requirements: decode_contract_requirements(self.contract_requirements)?,
            required_model_assets: self.required_model_assets,
        })
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
                .any(|contract| contract.id == "comfyui-py312-cu126-torch291"),
            "expected bundled ComfyUI endpoint contract"
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
