use std::collections::BTreeSet;

use super::{
    asset_text, assets, corrupt, execution_schemas::BundledExecutionSchemaRepository, parse_asset,
    runtime_contracts::BundledRuntimeContractRepository,
    runtime_presets::BundledRuntimePresetRepository,
};
use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{
        BundledModelAsset, BundledModelAssetDownloadSource, BundledReference, BundledWorkflow,
        BundledWorkflowContractRequirement, BundledWorkflowExecutionContract,
        BundledWorkflowInputBinding, ResolvedRunpodWorkflow,
    },
};

#[derive(Debug, Clone, Default)]
pub struct BundledWorkflowRepository;

impl BundledWorkflowRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledWorkflow>, BundledCatalogError> {
        workflow_revisions()
            .into_iter()
            .map(|(id, revision)| {
                self.get(&id, &revision)?.ok_or_else(|| {
                    corrupt(
                        &workflow_dir(&id, &revision),
                        "workflow revision disappeared",
                    )
                })
            })
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledWorkflow>, BundledCatalogError> {
        let metadata_path = workflow_file(id, revision, "metadata.json");
        let Some(metadata_text) = asset_text(&metadata_path) else {
            return Ok(None);
        };
        Ok(Some(parse_workflow(
            id,
            revision,
            &metadata_path,
            metadata_text,
        )?))
    }

    pub fn resolve_runpod_workflow(
        &self,
        id: &str,
        revision: &str,
        runtime_presets: &BundledRuntimePresetRepository,
        runtime_contracts: &BundledRuntimeContractRepository,
        execution_schemas: &BundledExecutionSchemaRepository,
    ) -> Result<Option<ResolvedRunpodWorkflow>, BundledCatalogError> {
        let Some(workflow) = self.get(id, revision)? else {
            return Ok(None);
        };
        let path = workflow_dir(id, revision);
        let runtime_preset = runtime_presets
            .get(
                &workflow.runtime_preset.id,
                &workflow.runtime_preset.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow runtime preset reference is missing"))?;
        let execution_schema = execution_schemas
            .get(
                &workflow.execution_contract.schema_ref.id,
                &workflow.execution_contract.schema_ref.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow execution schema reference is missing"))?;
        let runpod = workflow
            .contract_requirements
            .iter()
            .find(|requirement| requirement.runtime_type == "runpod")
            .ok_or_else(|| corrupt(&path, "workflow has no RunPod contract requirement"))?;
        let endpoint_contract = runtime_contracts
            .get(
                &runpod.endpoint_contract.id,
                &runpod.endpoint_contract.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow endpoint contract reference is missing"))?;
        let provisioner_contract = runtime_contracts
            .get(
                &runpod.provisioner_contract.id,
                &runpod.provisioner_contract.revision,
            )?
            .ok_or_else(|| corrupt(&path, "workflow provisioner contract reference is missing"))?;

        Ok(Some(ResolvedRunpodWorkflow {
            workflow,
            runtime_preset,
            execution_schema,
            endpoint_contract,
            provisioner_contract,
        }))
    }
}

fn parse_workflow(
    id: &str,
    revision: &str,
    metadata_path: &str,
    metadata_text: &str,
) -> Result<BundledWorkflow, BundledCatalogError> {
    let metadata = parse_asset::<generated::WorkflowMetadata>(metadata_path, metadata_text)?;
    let model_assets = parse_asset::<generated::WorkflowModelAssets>(
        &workflow_file(id, revision, "model_assets.json"),
        required_text(id, revision, "model_assets.json")?,
    )?;
    let contract_requirements = parse_asset::<generated::WorkflowContractRequirements>(
        &workflow_file(id, revision, "contract_requirements.json"),
        required_text(id, revision, "contract_requirements.json")?,
    )?;
    let execution_contract = parse_asset::<generated::WorkflowExecutionContract>(
        &workflow_file(id, revision, "execution_contract.json"),
        required_text(id, revision, "execution_contract.json")?,
    )?;
    let graph = parse_asset::<generated::WorkflowGraph>(
        &workflow_file(id, revision, "workflow.json"),
        required_text(id, revision, "workflow.json")?,
    )?;

    Ok(BundledWorkflow {
        id: metadata.id.into(),
        revision: metadata.revision.into(),
        name: metadata.name.into(),
        runtime_preset: BundledReference {
            id: metadata.runtime_preset.id.into(),
            revision: metadata.runtime_preset.revision.into(),
        },
        requires_hugging_face_api_key: metadata.requires_hugging_face_api_key,
        required_volume_size_gb: metadata.required_volume_size_gb.get(),
        model_assets: model_assets
            .model_assets
            .into_iter()
            .map(|asset| BundledModelAsset {
                id: asset.id.into(),
                name: asset.name.into(),
                download_source: BundledModelAssetDownloadSource {
                    source_type: "huggingface".to_string(),
                    repository_id: asset.download_source.repository_id.into(),
                    file_path: asset.download_source.file_path.into(),
                    revision: asset.download_source.revision.into(),
                },
                install_comfyui_relative_path: asset.install_comfyui_relative_path.into(),
            })
            .collect(),
        contract_requirements: contract_requirements
            .contract_requirements
            .into_iter()
            .map(|requirement| BundledWorkflowContractRequirement {
                runtime_type: "runpod".to_string(),
                endpoint_contract: reference(requirement.endpoint_contract),
                provisioner_contract: reference(requirement.provisioner_contract),
            })
            .collect(),
        execution_contract: BundledWorkflowExecutionContract {
            schema_ref: BundledReference {
                id: execution_contract.schema_ref.id.into(),
                revision: execution_contract.schema_ref.revision.into(),
            },
            input_bindings: execution_contract
                .input_bindings
                .into_iter()
                .map(|binding| BundledWorkflowInputBinding {
                    value: binding.value,
                    node_id: binding.node_id.into(),
                    path: binding.path.into_iter().map(Into::into).collect(),
                })
                .collect(),
        },
        graph: serde_json::Value::Object(graph.graph),
    })
}

fn reference(reference: generated::Reference) -> BundledReference {
    BundledReference {
        id: reference.id.into(),
        revision: reference.revision.into(),
    }
}

fn required_text(
    id: &str,
    revision: &str,
    file: &str,
) -> Result<&'static str, BundledCatalogError> {
    let path = workflow_file(id, revision, file);
    asset_text(&path).ok_or_else(|| corrupt(&path, "required workflow file is missing"))
}

fn workflow_revisions() -> Vec<(String, String)> {
    assets()
        .iter()
        .filter_map(|(path, _)| {
            let parts: Vec<&str> = path.split('/').collect();
            match parts.as_slice() {
                ["workflows", id, revision, "metadata.json"] => {
                    Some(((*id).to_string(), (*revision).to_string()))
                }
                _ => None,
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn workflow_file(id: &str, revision: &str, file: &str) -> String {
    format!("workflows/{id}/{revision}/{file}")
}

fn workflow_dir(id: &str, revision: &str) -> String {
    format!("workflows/{id}/{revision}")
}
