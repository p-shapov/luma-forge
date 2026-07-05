use std::collections::BTreeSet;

use super::{
    execution_schemas::BundledExecutionSchemaRepository,
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
        workflow_revisions()?
            .into_iter()
            .map(|(id, revision)| {
                self.get(&id, &revision)?.ok_or_else(|| {
                    BundledCatalogError::corrupt_asset(
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
        let Some(metadata_text) = generated::BUNDLED_ASSETS
            .iter()
            .find_map(|(asset_path, text)| (*asset_path == metadata_path).then_some(*text))
        else {
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
            .ok_or_else(|| {
                BundledCatalogError::corrupt_asset(
                    &path,
                    "workflow runtime preset reference is missing",
                )
            })?;
        let execution_schema = execution_schemas
            .get(
                &workflow.execution_contract.schema_ref.id,
                &workflow.execution_contract.schema_ref.revision,
            )?
            .ok_or_else(|| {
                BundledCatalogError::corrupt_asset(
                    &path,
                    "workflow execution schema reference is missing",
                )
            })?;
        let runpod = workflow
            .contract_requirements
            .iter()
            .find(|requirement| requirement.runtime_type == "runpod")
            .ok_or_else(|| {
                BundledCatalogError::corrupt_asset(
                    &path,
                    "workflow has no RunPod contract requirement",
                )
            })?;
        let endpoint_contract = runtime_contracts
            .get(
                &runpod.endpoint_contract.id,
                &runpod.endpoint_contract.revision,
            )?
            .ok_or_else(|| {
                BundledCatalogError::corrupt_asset(
                    &path,
                    "workflow endpoint contract reference is missing",
                )
            })?;
        let provisioner_contract = runtime_contracts
            .get(
                &runpod.provisioner_contract.id,
                &runpod.provisioner_contract.revision,
            )?
            .ok_or_else(|| {
                BundledCatalogError::corrupt_asset(
                    &path,
                    "workflow provisioner contract reference is missing",
                )
            })?;

        Ok(Some(ResolvedRunpodWorkflow {
            id: workflow.id,
            revision: workflow.revision,
            name: workflow.name,
            requires_hugging_face_api_key: workflow.requires_hugging_face_api_key,
            required_volume_size_gb: workflow.required_volume_size_gb,
            model_assets: workflow.model_assets,
            input_bindings: workflow.execution_contract.input_bindings,
            graph: workflow.graph,
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
        id: id.to_string(),
        revision: revision.to_string(),
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
    generated::BUNDLED_ASSETS
        .iter()
        .find_map(|(asset_path, text)| (*asset_path == path).then_some(*text))
        .ok_or_else(|| {
            BundledCatalogError::corrupt_asset(&path, "required workflow file is missing")
        })
}

fn parse_asset<T: serde::de::DeserializeOwned>(
    path: &str,
    text: &str,
) -> Result<T, BundledCatalogError> {
    serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))
}

fn workflow_identity_from_path(path: &str) -> Result<(String, String), BundledCatalogError> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["workflows", id, revision, _file] => Ok(((*id).to_string(), (*revision).to_string())),
        _ => Err(BundledCatalogError::corrupt_asset(
            path,
            "workflow path is invalid",
        )),
    }
}

fn workflow_revisions() -> Result<Vec<(String, String)>, BundledCatalogError> {
    generated::BUNDLED_ASSETS
        .iter()
        .filter_map(|(path, _)| {
            path.ends_with("/metadata.json")
                .then(|| workflow_identity_from_path(path))
        })
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|revisions| revisions.into_iter().collect())
}

fn workflow_file(id: &str, revision: &str, file: &str) -> String {
    format!("workflows/{id}/{revision}/{file}")
}

fn workflow_dir(id: &str, revision: &str) -> String {
    format!("workflows/{id}/{revision}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_uses_requested_workflow_identity() {
        let workflow = BundledWorkflowRepository::new()
            .get("comfyui-hidream-o1-dev", "1.0.0")
            .expect("lookup should succeed")
            .expect("workflow should exist");

        assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
        assert_eq!(workflow.revision, "1.0.0");
    }

    #[test]
    fn get_returns_none_for_missing_workflow() {
        let workflow = BundledWorkflowRepository::new()
            .get("missing-workflow", "9.9.9")
            .expect("lookup should succeed");

        assert_eq!(workflow, None);
    }
}
