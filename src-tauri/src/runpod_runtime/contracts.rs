use crate::domain::{
    runpod::RunpodContractRequirements,
    runtime_contract::{RuntimeCatalog, RuntimeContractReference},
    workflow_preset::{
        ModelAsset, WorkflowCatalog, WorkflowContractRequirements, WorkflowExecutionType,
        WorkflowReference, WorkflowRevision,
    },
};

use super::errors::{invalid_runtime_state_message, RunpodRuntimeError};
use crate::runtime_catalog::RuntimeCatalogService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeContract {
    pub id: String,
    pub version: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeContracts {
    pub endpoint_contract: RunpodRuntimeContract,
    pub provisioner_contract: RunpodRuntimeContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodWorkflowResolved {
    pub id: String,
    pub version: String,
    pub runtime_preset: String,
    pub _name: String,
    pub _execution_type: WorkflowExecutionType,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub contract_requirements: RunpodContractRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

pub struct RunpodWorkflowResolver;

impl RunpodWorkflowResolver {
    pub fn resolve_latest(
        catalog: &WorkflowCatalog,
        preset_id: &str,
    ) -> Option<RunpodWorkflowResolved> {
        let preset = catalog
            .workflow_presets
            .iter()
            .find(|preset| preset.id == preset_id)?;
        let revision = preset.revisions.last()?;
        resolve_workflow_revision(
            preset.id.clone(),
            preset.name.clone(),
            preset.execution_type,
            revision,
        )
    }

    pub fn resolve(
        catalog: &WorkflowCatalog,
        reference: &WorkflowReference,
    ) -> Option<RunpodWorkflowResolved> {
        let preset = catalog
            .workflow_presets
            .iter()
            .find(|preset| preset.id == reference.id)?;
        let revision = preset
            .revisions
            .iter()
            .find(|revision| revision.version == reference.version)?;
        resolve_workflow_revision(
            preset.id.clone(),
            preset.name.clone(),
            preset.execution_type,
            revision,
        )
    }
}

pub struct RunpodContractResolver;

impl RunpodContractResolver {
    pub fn resolve(
        workflow: &RunpodWorkflowResolved,
        runtime_catalog: &RuntimeCatalogService,
    ) -> Result<RunpodRuntimeContracts, RunpodRuntimeError> {
        let runtime_catalog = runtime_catalog
            .get_runtime_contract_catalog()
            .map_err(RunpodRuntimeError::from)?;

        let endpoint_contract = runtime_catalog
            .resolve(&workflow.contract_requirements.endpoint_contract)
            .ok_or_else(|| {
                invalid_runtime_state_message("endpoint runtime contract was not found")
            })?;
        let provisioner_contract = runtime_catalog
            .resolve(&workflow.contract_requirements.provisioner_contract)
            .ok_or_else(|| {
                invalid_runtime_state_message("provisioner runtime contract was not found")
            })?;

        Ok(RunpodRuntimeContracts {
            endpoint_contract,
            provisioner_contract,
        })
    }
}

impl RuntimeCatalog {
    fn resolve(&self, reference: &RuntimeContractReference) -> Option<RunpodRuntimeContract> {
        let contract = self
            .contracts
            .iter()
            .find(|contract| contract.id == reference.id)?;
        let revision = contract
            .revisions
            .iter()
            .find(|revision| revision.version == reference.version)?;

        Some(RunpodRuntimeContract {
            id: contract.id.clone(),
            version: revision.version.clone(),
            image_ref: revision.image_ref.clone(),
        })
    }
}

fn resolve_workflow_revision(
    id: String,
    name: String,
    execution_type: WorkflowExecutionType,
    revision: &WorkflowRevision,
) -> Option<RunpodWorkflowResolved> {
    let contract_requirements = revision
        .contract_requirements
        .iter()
        .map(|requirements| match requirements {
            WorkflowContractRequirements::Runpod(requirements) => requirements,
        })
        .next()?
        .clone();

    Some(RunpodWorkflowResolved {
        id,
        version: revision.version.clone(),
        runtime_preset: revision.runtime_preset.clone(),
        _name: name,
        _execution_type: execution_type,
        requires_hugging_face_api_key: revision.requires_hugging_face_api_key,
        required_volume_size_gb: revision.required_volume_size_gb,
        contract_requirements,
        required_model_assets: revision.required_model_assets.clone(),
    })
}
