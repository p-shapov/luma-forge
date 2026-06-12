use crate::domain::{
    provisioned_remote::ProvisionedRemoteRuntime, runtime_contract::RuntimeContractResolved,
    workflow_preset::WorkflowPresetResolved,
};

use super::errors::ProvisionedRemoteError;
use crate::workflow_catalog::reader::{
    BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedRemoteRuntimeContracts {
    pub endpoint_contract: RuntimeContractResolved,
    pub provisioner_contract: RuntimeContractResolved,
}

pub struct ProvisionedRemoteContractResolver;

impl ProvisionedRemoteContractResolver {
    pub fn resolve(
        workflow: &WorkflowPresetResolved,
        _runtime: &ProvisionedRemoteRuntime,
    ) -> Result<ProvisionedRemoteRuntimeContracts, ProvisionedRemoteError> {
        let endpoint_catalog = BundledEndpointContractCatalogReader
            .read_endpoint_contract_catalog()
            .map_err(|_| ProvisionedRemoteError::InvalidRuntimeState)?;
        let provisioner_catalog = BundledProvisionerContractCatalogReader
            .read_provisioner_contract_catalog()
            .map_err(|_| ProvisionedRemoteError::InvalidRuntimeState)?;

        let endpoint_contract = endpoint_catalog
            .resolve(&workflow.runpod_runtime_requirements.endpoint_contract)
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;
        let provisioner_contract = provisioner_catalog
            .resolve(&workflow.runpod_runtime_requirements.provisioner_contract)
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;

        Ok(ProvisionedRemoteRuntimeContracts {
            endpoint_contract,
            provisioner_contract,
        })
    }
}
