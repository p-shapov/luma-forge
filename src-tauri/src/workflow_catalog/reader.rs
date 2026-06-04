use crate::domain::{runtime_contract::RuntimeCatalog, workflow_preset::WorkflowPreset};

use super::WorkflowCatalogError;

pub trait WorkflowCatalogReader {
    fn read_workflows(&self) -> Result<Vec<WorkflowPreset>, WorkflowCatalogError>;
}

pub trait EndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError>;
}

pub trait ProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> Result<RuntimeCatalog, WorkflowCatalogError>;
}
