use crate::domain::{runtime_contract::RuntimeCatalog, workflow_preset::WorkflowPreset};

use super::WorkflowCatalogResult;

pub trait WorkflowCatalogReader {
    fn read_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>>;
}

pub trait EndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

pub trait ProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledWorkflowCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledEndpointContractCatalogReader;

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledProvisionerContractCatalogReader;
