use crate::domain::{
    provisioned_remote::ProvisionedRemoteRuntime, runtime_contract::RuntimeContractReference,
    workspace::Workspace,
};

use super::errors::ProvisionedRemoteError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedRemoteRuntimeContracts {
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
}

pub struct ProvisionedRemoteContractResolver;

impl ProvisionedRemoteContractResolver {
    pub fn resolve(
        workspace: &Workspace,
        runtime: &ProvisionedRemoteRuntime,
    ) -> Result<ProvisionedRemoteRuntimeContracts, ProvisionedRemoteError> {
        let provider_requirements = workspace
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(runtime.provider_id())
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;

        Ok(ProvisionedRemoteRuntimeContracts {
            endpoint_contract: provider_requirements.endpoint_contract.clone(),
            provisioner_contract: provider_requirements.provisioner_contract.clone(),
        })
    }
}
