use std::collections::HashSet;

use serde_json::Value;

use crate::domain::workflow_preset::{
    ModelAsset, ModelAssetSource, WorkflowPreset, WorkflowRevision,
};

use super::execution_schemas::{
    input_ids, required_input_ids, ExecutionSchemaRegistry,
};
use super::errors::WorkflowCatalogError;

const EMPTY_WORKFLOWS: &str = "workflows are empty";
const INVALID_WORKFLOW_ID: &str = "workflow ID is empty, duplicate, or name is empty";
const EMPTY_WORKFLOW_REVISIONS: &str = "workflow has no revisions";
const INVALID_WORKFLOW_REVISION_VERSION: &str = "revision version is empty or duplicate";
const INVALID_RUNTIME_PRESET: &str = "runtime preset is empty";
const ZERO_REQUIRED_VOLUME_SIZE: &str = "required volume size is zero";
const EMPTY_CONTRACT_REQUIREMENTS: &str = "contract requirements are empty";
const INVALID_MODEL_ASSET: &str =
    "model asset ID, name, install path, or download source is invalid";

pub(super) fn validate_workflows(
    workflows: &[WorkflowPreset],
    execution_schemas: &ExecutionSchemaRegistry,
) -> Result<(), WorkflowCatalogError> {
    if workflows.is_empty() {
        return validation_error(EMPTY_WORKFLOWS);
    }

    let mut workflow_ids = HashSet::new();
    for workflow in workflows {
        validate_workflow(workflow, &mut workflow_ids, execution_schemas)?;
    }

    Ok(())
}

fn validate_workflow<'catalog>(
    workflow: &'catalog WorkflowPreset,
    workflow_ids: &mut HashSet<&'catalog str>,
    execution_schemas: &ExecutionSchemaRegistry,
) -> Result<(), WorkflowCatalogError> {
    if workflow.id.trim().is_empty()
        || !workflow_ids.insert(workflow.id.as_str())
        || workflow.name.trim().is_empty()
    {
        return validation_error(INVALID_WORKFLOW_ID);
    }

    if workflow.revisions.is_empty() {
        return validation_error(EMPTY_WORKFLOW_REVISIONS);
    }

    let mut revision_versions = HashSet::new();
    for revision in &workflow.revisions {
        validate_workflow_revision(revision, &mut revision_versions, execution_schemas)?;
    }

    Ok(())
}

fn validate_workflow_revision<'workflow>(
    revision: &'workflow WorkflowRevision,
    revision_versions: &mut HashSet<&'workflow str>,
    execution_schemas: &ExecutionSchemaRegistry,
) -> Result<(), WorkflowCatalogError> {
    if revision.version.trim().is_empty() || !revision_versions.insert(revision.version.as_str()) {
        return validation_error(INVALID_WORKFLOW_REVISION_VERSION);
    }

    if revision.runtime_preset.trim().is_empty() {
        return validation_error(INVALID_RUNTIME_PRESET);
    }

    validate_runtime_requirements_shape(revision)?;
    validate_execution_contract(revision, execution_schemas)?;

    for asset in &revision.required_model_assets {
        validate_model_asset(asset)?;
    }

    Ok(())
}

fn validate_execution_contract(
    revision: &WorkflowRevision,
    execution_schemas: &ExecutionSchemaRegistry,
) -> Result<(), WorkflowCatalogError> {
    let schema_ref = &revision.execution_contract.schema_ref;
    let Some(schema_revision) = execution_schemas.find_revision(&schema_ref.id, &schema_ref.version)
    else {
        return validation_error("execution contract schema reference is invalid");
    };
    if revision.execution_contract.input_bindings.is_empty() {
        return validation_error("execution contract input bindings are empty");
    }

    let valid_inputs = input_ids(schema_revision);
    let mut bound_inputs = HashSet::new();
    for binding in &revision.execution_contract.input_bindings {
        if binding.node_id.trim().is_empty() || binding.path.is_empty() {
            return validation_error("execution contract input binding target is invalid");
        }
        if let Some(input_id) = template_input_id(&binding.value)? {
            if !valid_inputs.contains(input_id) {
                return validation_error("execution contract input binding references unknown input");
            }
            bound_inputs.insert(input_id.to_string());
        }
    }

    for required in required_input_ids(schema_revision) {
        if !bound_inputs.contains(required) {
            return validation_error("execution contract missing required input binding");
        }
    }

    Ok(())
}

fn template_input_id(value: &Value) -> Result<Option<&str>, WorkflowCatalogError> {
    let Value::String(text) = value else {
        return Ok(None);
    };
    if !text.starts_with("{{") && !text.ends_with("}}") {
        return Ok(None);
    }
    if !(text.starts_with("{{") && text.ends_with("}}")) || text.len() <= 4 {
        return validation_error("execution contract input binding template is malformed");
    }
    let inner = &text[2..text.len() - 2];
    if inner.trim() != inner || inner.is_empty() || inner.contains('{') || inner.contains('}') {
        return validation_error("execution contract input binding template is malformed");
    }
    Ok(Some(inner))
}

fn validate_runtime_requirements_shape(
    revision: &WorkflowRevision,
) -> Result<(), WorkflowCatalogError> {
    if revision.required_volume_size_gb == 0 {
        return validation_error(ZERO_REQUIRED_VOLUME_SIZE);
    }

    if revision.contract_requirements.is_empty() {
        return validation_error(EMPTY_CONTRACT_REQUIREMENTS);
    }

    Ok(())
}

fn validate_model_asset(asset: &ModelAsset) -> Result<(), WorkflowCatalogError> {
    if asset.id.trim().is_empty()
        || asset.name.trim().is_empty()
        || !is_safe_relative_path(&asset.install_comfyui_relative_path)
        || !is_valid_model_asset_source(&asset.download_source)
    {
        return validation_error(INVALID_MODEL_ASSET);
    }

    Ok(())
}

fn is_valid_model_asset_source(source: &ModelAssetSource) -> bool {
    match source {
        ModelAssetSource::Huggingface {
            repository_id,
            file_path,
            revision,
        } => {
            is_valid_hugging_face_repository_id(repository_id)
                && is_safe_relative_path(file_path)
                && !revision.trim().is_empty()
        }
    }
}

fn is_valid_hugging_face_repository_id(repository_id: &str) -> bool {
    let Some((owner, repository)) = repository_id.split_once('/') else {
        return false;
    };

    !repository.contains('/')
        && is_safe_hugging_face_name(owner)
        && is_safe_hugging_face_name(repository)
}

fn is_safe_hugging_face_name(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn is_safe_relative_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn validation_error<T>(message: &'static str) -> Result<T, WorkflowCatalogError> {
    Err(WorkflowCatalogError::ValidationFailed {
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        runpod::RunpodContractRequirements,
        runtime_contract::RuntimeContractReference,
        workflow_preset::{
            ExecutionContract, ExecutionSchemaReference, InputBinding, ModelAsset,
            ModelAssetSource, WorkflowContractRequirements, WorkflowRevision,
        },
    };

    fn valid_asset() -> ModelAsset {
        ModelAsset {
            id: "hidream-o1-image-dev-fp8-scaled".to_string(),
            name: "HiDream O1 Image Dev FP8 Scaled".to_string(),
            download_source: ModelAssetSource::Huggingface {
                repository_id: "Comfy-Org/HiDream-O1-Image".to_string(),
                file_path: "checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors".to_string(),
                revision: "e469681accde36057e32e4a3125e39929a1bcd68".to_string(),
            },
            install_comfyui_relative_path:
                "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors".to_string(),
        }
    }

    fn valid_revision(version: &str) -> WorkflowRevision {
        WorkflowRevision {
            version: version.to_string(),
            runtime_preset: "comfyui-py312-cu126-torch291".to_string(),
            execution_contract: valid_execution_contract(),
            requires_hugging_face_api_key: true,
            required_volume_size_gb: 19,
            contract_requirements: vec![WorkflowContractRequirements::Runpod(
                RunpodContractRequirements {
                    endpoint_contract: RuntimeContractReference {
                        id: "runpod-endpoint-comfyui-hidream-o1-dev".to_string(),
                        version: "1.0.15".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner".to_string(),
                        version: "1.0.6".to_string(),
                    },
                },
            )],
            required_model_assets: vec![valid_asset()],
        }
    }

    fn valid_execution_contract() -> ExecutionContract {
        ExecutionContract {
            schema_ref: ExecutionSchemaReference {
                id: "text-to-image".to_string(),
                version: "1.0.0".to_string(),
            },
            input_bindings: vec![
                InputBinding {
                    value: serde_json::Value::String("{{prompt}}".to_string()),
                    node_id: "171".to_string(),
                    path: vec!["widgets_values".to_string(), "0".to_string()],
                },
                InputBinding {
                    value: serde_json::Value::Bool(false),
                    node_id: "154".to_string(),
                    path: vec!["widgets_values".to_string(), "0".to_string()],
                },
                InputBinding {
                    value: serde_json::Value::Bool(false),
                    node_id: "177".to_string(),
                    path: vec!["widgets_values".to_string(), "0".to_string()],
                },
            ],
        }
    }

    fn valid_workflow(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            name: "ComfyUI HiDream O1 Dev".to_string(),
            revisions: vec![valid_revision("1.0.0")],
        }
    }

    fn execution_schemas() -> crate::workflow_catalog::execution_schemas::ExecutionSchemaRegistry {
        crate::workflow_catalog::execution_schemas::ExecutionSchemaRegistry {
            execution_schemas: vec![crate::workflow_catalog::execution_schemas::ExecutionSchema {
                id: "text-to-image".to_string(),
                revisions: vec![
                    crate::workflow_catalog::execution_schemas::ExecutionSchemaRevision {
                        version: "1.0.0".to_string(),
                        inputs: vec![
                            crate::workflow_catalog::execution_schemas::ExecutionSchemaInput {
                                id: "prompt".to_string(),
                                input_type: "string".to_string(),
                                required: true,
                                max_length: Some(4000),
                            },
                        ],
                        outputs: crate::workflow_catalog::execution_schemas::ExecutionSchemaOutputs {
                            output_type: "image_set".to_string(),
                        },
                    },
                ],
            }],
        }
    }

    fn validate_test_workflows(workflows: &[WorkflowPreset]) -> Result<(), WorkflowCatalogError> {
        validate_workflows(workflows, &execution_schemas())
    }

    #[test]
    fn validate_workflows_accepts_valid_workflow() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];
        assert_eq!(validate_test_workflows(&workflows), Ok(()));
    }

    #[test]
    fn validate_workflows_rejects_duplicate_workflow_ids() {
        let workflows = vec![
            valid_workflow("comfyui-hidream-o1-dev"),
            valid_workflow("comfyui-hidream-o1-dev"),
        ];
        assert_eq!(
            validate_test_workflows(&workflows),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "workflow ID is empty, duplicate, or name is empty".to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_empty_workflow_revisions() {
        let mut workflow = valid_workflow("workflow");
        workflow.revisions.clear();

        assert_eq!(
            validate_test_workflows(&[workflow]),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "workflow has no revisions".to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_duplicate_revision_versions() {
        let mut workflow = valid_workflow("workflow");
        workflow.revisions.push(workflow.revisions[0].clone());

        assert_eq!(
            validate_test_workflows(&[workflow]),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "revision version is empty or duplicate".to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_zero_required_volume_size_gb() {
        let mut workflow = valid_workflow("workflow");
        workflow.revisions[0].required_volume_size_gb = 0;

        assert_eq!(
            validate_test_workflows(&[workflow]),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "required volume size is zero".to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_invalid_model_asset_paths() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.revisions[0].required_model_assets[0].install_comfyui_relative_path =
            "../outside.safetensors".to_string();
        let workflows = vec![workflow];
        assert_eq!(
            validate_test_workflows(&workflows),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "model asset ID, name, install path, or download source is invalid"
                    .to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_invalid_model_asset_source_paths() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.revisions[0].required_model_assets[0].download_source =
            ModelAssetSource::Huggingface {
                repository_id: "Comfy-Org/HiDream-O1-Image".to_string(),
                file_path: "../hidream.safetensors".to_string(),
                revision: "e469681accde36057e32e4a3125e39929a1bcd68".to_string(),
            };
        let workflows = vec![workflow];

        assert_eq!(
            validate_test_workflows(&workflows),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "model asset ID, name, install path, or download source is invalid"
                    .to_string()
            })
        );
    }

    #[test]
    fn validate_workflows_rejects_missing_required_execution_binding() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.revisions[0].execution_contract.input_bindings.clear();

        assert_eq!(
            validate_test_workflows(&[workflow]),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "execution contract input bindings are empty".to_string()
            })
        );
    }
}
