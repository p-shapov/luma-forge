use serde::{Deserialize, Serialize};

pub mod validator;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerCatalog {
    pub contracts: Vec<ProvisionerContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerContract {
    pub id: String,
    pub revisions: Vec<ProvisionerContractRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerContractRevision {
    pub version: String,
    pub provisioner_worker_image_ref: String,
    pub volume_mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerContractReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProvisionerImageSnapshot {
    pub contract_id: String,
    pub contract_version: String,
    pub provisioner_worker_image_ref: String,
    pub volume_mount_path: String,
}

impl ProvisionerCatalog {
    pub fn resolve(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<ResolvedProvisionerImageSnapshot> {
        let contract = self
            .contracts
            .iter()
            .find(|contract| contract.id == contract_id)?;
        let revision = contract
            .revisions
            .iter()
            .find(|revision| revision.version == contract_version)?;

        Some(ResolvedProvisionerImageSnapshot {
            contract_id: contract.id.clone(),
            contract_version: revision.version.clone(),
            provisioner_worker_image_ref: revision.provisioner_worker_image_ref.clone(),
            volume_mount_path: revision.volume_mount_path.clone(),
        })
    }
}
