use crate::domain::{
    runtime_contract::RuntimeContractResolved, workflow_preset::WorkflowPresetResolved,
};

use super::errors::RunpodRuntimeError;
use crate::workflow_catalog::WorkflowCatalogService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeContracts {
    pub endpoint_contract: RuntimeContractResolved,
    pub provisioner_contract: RuntimeContractResolved,
}

pub struct RunpodContractResolver;

impl RunpodContractResolver {
    pub fn resolve(
        workflow: &WorkflowPresetResolved,
        workflow_catalog: &WorkflowCatalogService,
    ) -> Result<RunpodRuntimeContracts, RunpodRuntimeError> {
        let endpoint_catalog = workflow_catalog
            .get_endpoint_contract_catalog()
            .map_err(|_| RunpodRuntimeError::InvalidRuntimeState)?;
        let provisioner_catalog = workflow_catalog
            .get_provisioner_contract_catalog()
            .map_err(|_| RunpodRuntimeError::InvalidRuntimeState)?;

        let endpoint_contract = endpoint_catalog
            .resolve(&workflow.runpod_runtime_requirements.endpoint_contract)
            .ok_or(RunpodRuntimeError::InvalidRuntimeState)?;
        let provisioner_contract = provisioner_catalog
            .resolve(&workflow.runpod_runtime_requirements.provisioner_contract)
            .ok_or(RunpodRuntimeError::InvalidRuntimeState)?;

        Ok(RunpodRuntimeContracts {
            endpoint_contract,
            provisioner_contract,
        })
    }
}
