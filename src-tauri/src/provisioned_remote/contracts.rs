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
        runtime: &ProvisionedRemoteRuntime,
    ) -> Result<ProvisionedRemoteRuntimeContracts, ProvisionedRemoteError> {
        let provider_requirements = workflow
            .remote_runtime_requirements
            .resolve_provider_requirements(runtime.provider_id())
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;

        let endpoint_catalog = BundledEndpointContractCatalogReader
            .read_endpoint_contract_catalog()
            .map_err(|_| ProvisionedRemoteError::InvalidRuntimeState)?;
        let provisioner_catalog = BundledProvisionerContractCatalogReader
            .read_provisioner_contract_catalog()
            .map_err(|_| ProvisionedRemoteError::InvalidRuntimeState)?;

        let endpoint_contract = endpoint_catalog
            .resolve(&provider_requirements.endpoint_contract)
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;
        let provisioner_contract = provisioner_catalog
            .resolve(&provider_requirements.provisioner_contract)
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;

        Ok(ProvisionedRemoteRuntimeContracts {
            endpoint_contract,
            provisioner_contract,
        })
    }
}
