use super::{CatalogEntry, Documents};
use crate::infra::bundled::{errors::BundledCatalogError, generated, Catalog};

pub struct Entry;

#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub metadata: generated::WorkflowMetadata,
    pub model_assets: generated::WorkflowModelAssets,
    pub contract_requirements: generated::WorkflowContractRequirements,
    pub execution_contract: generated::WorkflowExecutionContract,
    pub workflow_graph: generated::WorkflowGraph,
}

impl Entry {
    pub async fn all(catalog: &Catalog) -> Result<Vec<Model>, BundledCatalogError> {
        catalog.all::<Self>().await
    }

    pub async fn get(
        catalog: &Catalog,
        key: (&str, &str),
    ) -> Result<Option<Model>, BundledCatalogError> {
        catalog.get::<Self>(key).await
    }
}

impl CatalogEntry for Entry {
    type Model = Model;

    const CONTRACT: &'static str = "catalog/contracts/workflow_revision";

    fn decode(
        id: String,
        revision: String,
        mut documents: Documents,
    ) -> Result<Model, BundledCatalogError> {
        Ok(Model {
            id,
            revision,
            metadata: documents.take("metadata")?,
            model_assets: documents.take("model_assets")?,
            contract_requirements: documents.take("contract_requirements")?,
            execution_contract: documents.take("execution_contract")?,
            workflow_graph: documents.take("workflow")?,
        })
    }
}
