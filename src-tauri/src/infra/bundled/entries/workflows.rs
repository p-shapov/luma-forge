use std::collections::HashMap;

use serde_json::Value;

use super::{parse_document, CatalogEntry, Select};
use crate::infra::bundled::{errors::BundledCatalogError, generated};

#[derive(Debug)]
pub struct Entry {
    pub id: String,
    pub revision: String,
    pub metadata: generated::WorkflowMetadata,
    pub model_assets: generated::WorkflowModelAssets,
    pub contract_requirements: generated::WorkflowContractRequirements,
    pub execution_contract: generated::WorkflowExecutionContract,
    pub workflow_graph: generated::WorkflowGraph,
}

impl Entry {
    pub fn find() -> Select<Self> {
        Select::find()
    }

    pub fn find_by_id(key: (&str, &str)) -> Select<Self> {
        Select::find_by_id(key)
    }
}

impl CatalogEntry for Entry {
    const ENTITY: &'static str = "workflow_revision";

    fn from_documents(
        id: String,
        revision: String,
        relative: String,
        mut documents: HashMap<String, Value>,
    ) -> Result<Self, BundledCatalogError> {
        Ok(Self {
            id,
            revision,
            metadata: parse_document(&mut documents, &relative, "metadata.json")?,
            model_assets: parse_document(&mut documents, &relative, "model_assets.json")?,
            contract_requirements: parse_document(
                &mut documents,
                &relative,
                "contract_requirements.json",
            )?,
            execution_contract: parse_document(
                &mut documents,
                &relative,
                "execution_contract.json",
            )?,
            workflow_graph: parse_document(&mut documents, &relative, "workflow.json")?,
        })
    }
}
