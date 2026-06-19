use crate::{
    domain::{
        runpod::RunpodContractRequirements,
        runtime_contract::{RuntimeCatalog, RuntimeContractReference},
        workflow_preset::{
            ModelAsset, WorkflowCatalog, WorkflowContractRequirements, WorkflowReference,
        },
    },
    runtime_catalog::RuntimeCatalogRepository,
    workspace::{errors::invalid_state, WorkspaceError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunpodRuntimeContract {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunpodRuntimeContracts {
    pub(crate) endpoint_contract: RunpodRuntimeContract,
    pub(crate) provisioner_contract: RunpodRuntimeContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunpodWorkflowResolved {
    pub(crate) requires_hugging_face_api_key: bool,
    pub(crate) contract_requirements: RunpodContractRequirements,
    pub(crate) required_model_assets: Vec<ModelAsset>,
}

pub(crate) struct RunpodWorkflowResolver;

impl RunpodWorkflowResolver {
    pub(crate) fn resolve(
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
        let contract_requirements = revision
            .contract_requirements
            .iter()
            .map(|requirements| match requirements {
                WorkflowContractRequirements::Runpod(requirements) => requirements,
            })
            .next()?
            .clone();

        Some(RunpodWorkflowResolved {
            requires_hugging_face_api_key: revision.requires_hugging_face_api_key,
            contract_requirements,
            required_model_assets: revision.required_model_assets.clone(),
        })
    }
}

pub(crate) fn resolve_contracts(
    workflow: &RunpodWorkflowResolved,
    runtime_catalog: &impl RuntimeCatalogRepository,
) -> Result<RunpodRuntimeContracts, WorkspaceError> {
    let runtime_catalog = runtime_catalog.get_runtime_contract_catalog()?;

    let endpoint_contract = resolve_runtime_contract(
        &runtime_catalog,
        &workflow.contract_requirements.endpoint_contract,
    )
    .ok_or_else(|| invalid_state("endpoint runtime contract was not found"))?;
    let provisioner_contract = resolve_runtime_contract(
        &runtime_catalog,
        &workflow.contract_requirements.provisioner_contract,
    )
    .ok_or_else(|| invalid_state("provisioner runtime contract was not found"))?;

    Ok(RunpodRuntimeContracts {
        endpoint_contract,
        provisioner_contract,
    })
}

fn resolve_runtime_contract(
    runtime_catalog: &RuntimeCatalog,
    reference: &RuntimeContractReference,
) -> Option<RunpodRuntimeContract> {
    let contract = runtime_catalog
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
