use crate::{
    application::{
        catalog::{
            CatalogRef, RunpodContractRequirements, RuntimeContractRequirements,
            WorkflowDefinition, WorkflowSummary,
        },
        workspace::ports::{WorkflowCatalog, WorkflowCatalogError},
    },
    infra::bundled::{
        entries::workflows,
        errors::BundledCatalogError,
        generated::{Reference, WorkflowContractRequirementsContractRequirementsItem},
    },
};

use super::BundledCatalogAdapter;

const RUNTIME_PRESET_CONTRACT: &str = "catalog/contracts/runtime_preset_revision";
const RUNTIME_CONTRACT: &str = "catalog/contracts/runtime_contract_revision";

#[async_trait::async_trait]
impl WorkflowCatalog for BundledCatalogAdapter {
    async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError> {
        let mut summaries = workflows::Entry::all(&self.catalog)
            .await
            .map_err(map_catalog_error)?
            .into_iter()
            .map(workflow_definition)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|definition| definition.summary)
            .collect::<Vec<_>>();
        summaries
            .sort_by(|left, right| (&left.id, &left.revision).cmp(&(&right.id, &right.revision)));
        Ok(summaries)
    }

    async fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError> {
        workflows::Entry::get(&self.catalog, (id, revision))
            .await
            .map_err(map_catalog_error)?
            .map(workflow_definition)
            .transpose()
    }
}

fn workflow_definition(
    workflow: workflows::Model,
) -> Result<WorkflowDefinition, WorkflowCatalogError> {
    let metadata = workflow.metadata;
    let runtime_preset_ref = catalog_ref(metadata.runtime_preset_ref, RUNTIME_PRESET_CONTRACT)?;
    let contract_requirements = workflow
        .contract_requirements
        .contract_requirements
        .into_iter()
        .map(|requirement| match requirement {
            WorkflowContractRequirementsContractRequirementsItem::Runpod {
                endpoint_contract_ref,
                provisioner_contract_ref,
            } => Ok(RuntimeContractRequirements::Runpod(
                RunpodContractRequirements {
                    provisioner_contract_ref: catalog_ref(
                        provisioner_contract_ref,
                        RUNTIME_CONTRACT,
                    )?,
                    endpoint_contract_ref: catalog_ref(endpoint_contract_ref, RUNTIME_CONTRACT)?,
                },
            )),
        })
        .collect::<Result<Vec<_>, WorkflowCatalogError>>()?;

    Ok(WorkflowDefinition {
        summary: WorkflowSummary {
            id: workflow.id,
            revision: workflow.revision,
            name: String::from(metadata.name),
            description: String::from(metadata.description),
            required_volume_size_gb: metadata.required_volume_size_gb.get(),
            requires_hugging_face_api_key: metadata.requires_hugging_face_api_key,
        },
        runtime_preset_ref,
        contract_requirements,
        model_assets: serde_json::to_value(workflow.model_assets.model_assets)
            .map_err(|_| WorkflowCatalogError::InvalidCatalog)?,
        execution_contract: serde_json::to_value(workflow.execution_contract)
            .map_err(|_| WorkflowCatalogError::InvalidCatalog)?,
        workflow_graph: serde_json::to_value(workflow.workflow_graph)
            .map_err(|_| WorkflowCatalogError::InvalidCatalog)?,
    })
}

fn catalog_ref(reference: Reference, contract: &str) -> Result<CatalogRef, WorkflowCatalogError> {
    if reference.contract.as_str() != contract {
        return Err(WorkflowCatalogError::InvalidCatalog);
    }
    Ok(CatalogRef::new(
        String::from(reference.id),
        String::from(reference.revision),
    ))
}

fn map_catalog_error(error: BundledCatalogError) -> WorkflowCatalogError {
    match error {
        BundledCatalogError::Io { .. } => WorkflowCatalogError::Unavailable,
        BundledCatalogError::Json { .. }
        | BundledCatalogError::Contract { .. }
        | BundledCatalogError::Entry { .. } => WorkflowCatalogError::InvalidCatalog,
    }
}
