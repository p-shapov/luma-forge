use serde::Deserialize;
use serde_json::Value;

use crate::domain::workflow_preset::{
    ExecutionContract, ModelAsset, WorkflowCatalog, WorkflowPreset, WorkflowRevision,
};

use super::{contract_requirements::decode_contract_requirements, WorkflowCatalogError};

const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledWorkflowCatalogReader;

impl BundledWorkflowCatalogReader {
    pub fn read_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
        let document: WorkflowCatalogDocument =
            serde_json::from_str(WORKFLOW_CATALOG_JSON).map_err(parse_error)?;

        document.decode()
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
    revisions: Vec<WorkflowRevisionDocument>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRevisionDocument {
    version: String,
    runtime_preset: String,
    execution_contract: ExecutionContract,
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
            runtime_preset: self.runtime_preset,
            execution_contract: self.execution_contract,
            requires_hugging_face_api_key: self.requires_hugging_face_api_key,
            required_volume_size_gb: self.required_volume_size_gb,
            contract_requirements: decode_contract_requirements(self.contract_requirements)?,
            required_model_assets: self.required_model_assets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BundledWorkflowCatalogReader;

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

        let revision = workflows
            .workflow_presets
            .iter()
            .find(|workflow| workflow.id == "comfyui-hidream-o1-dev")
            .and_then(|workflow| {
                workflow
                    .revisions
                    .iter()
                    .find(|revision| revision.version == "1.0.0")
            })
            .expect("expected HiDream revision");

        assert_eq!(revision.execution_contract.schema_ref.id, "text-to-image");
        assert_eq!(revision.execution_contract.schema_ref.version, "1.0.0");
        assert_eq!(revision.execution_contract.input_bindings.len(), 3);
    }
}
