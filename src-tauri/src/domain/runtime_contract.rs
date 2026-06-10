use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCatalog {
    pub contracts: Vec<RuntimeContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContract {
    pub id: String,
    pub revisions: Vec<RuntimeContractRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractRevision {
    pub version: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractResolved {
    pub id: String,
    pub version: String,
    pub image_ref: String,
}

impl RuntimeCatalog {
    pub fn resolve(&self, reference: &RuntimeContractReference) -> Option<RuntimeContractResolved> {
        let contract = self
            .contracts
            .iter()
            .find(|contract| contract.id == reference.id)?;
        let revision = contract
            .revisions
            .iter()
            .find(|revision| revision.version == reference.version)?;

        Some(RuntimeContractResolved {
            id: contract.id.clone(),
            version: revision.version.clone(),
            image_ref: revision.image_ref.clone(),
        })
    }
}
