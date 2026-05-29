use serde::{Deserialize, Serialize};

pub mod validator;

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
    pub endpoint_image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRuntimeImageSnapshot {
    pub contract_id: String,
    pub contract_version: String,
    pub endpoint_image_ref: String,
}

impl RuntimeCatalog {
    pub fn resolve(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<ResolvedRuntimeImageSnapshot> {
        let contract = self
            .contracts
            .iter()
            .find(|contract| contract.id == contract_id)?;
        let revision = contract
            .revisions
            .iter()
            .find(|revision| revision.version == contract_version)?;

        Some(ResolvedRuntimeImageSnapshot {
            contract_id: contract.id.clone(),
            contract_version: revision.version.clone(),
            endpoint_image_ref: revision.endpoint_image_ref.clone(),
        })
    }
}
