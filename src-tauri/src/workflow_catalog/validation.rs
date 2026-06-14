use std::collections::HashSet;

use crate::domain::workflow_preset::{
    ModelAsset, ModelAssetSource, WorkflowPreset, WorkflowRevision,
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

pub(super) fn validate_workflows(workflows: &[WorkflowPreset]) -> Result<(), WorkflowCatalogError> {
    if workflows.is_empty() {
        return validation_error(EMPTY_WORKFLOWS);
    }

    let mut workflow_ids = HashSet::new();
    for workflow in workflows {
        validate_workflow(workflow, &mut workflow_ids)?;
    }

    Ok(())
}

fn validate_workflow<'catalog>(
    workflow: &'catalog WorkflowPreset,
    workflow_ids: &mut HashSet<&'catalog str>,
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
        validate_workflow_revision(revision, &mut revision_versions)?;
    }

    Ok(())
}

fn validate_workflow_revision<'workflow>(
    revision: &'workflow WorkflowRevision,
    revision_versions: &mut HashSet<&'workflow str>,
) -> Result<(), WorkflowCatalogError> {
    if revision.version.trim().is_empty() || !revision_versions.insert(revision.version.as_str()) {
        return validation_error(INVALID_WORKFLOW_REVISION_VERSION);
    }

    if revision.runtime_preset.trim().is_empty() {
        return validation_error(INVALID_RUNTIME_PRESET);
    }

    validate_runtime_requirements_shape(revision)?;

    for asset in &revision.required_model_assets {
        validate_model_asset(asset)?;
    }

    Ok(())
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
            ModelAsset, ModelAssetSource, WorkflowContractRequirements, WorkflowRevision,
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

    fn valid_workflow(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            name: "ComfyUI HiDream O1 Dev".to_string(),
            revisions: vec![valid_revision("1.0.0")],
        }
    }

    #[test]
    fn validate_workflows_accepts_valid_workflow() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];
        assert_eq!(validate_workflows(&workflows), Ok(()));
    }

    #[test]
    fn validate_workflows_rejects_duplicate_workflow_ids() {
        let workflows = vec![
            valid_workflow("comfyui-hidream-o1-dev"),
            valid_workflow("comfyui-hidream-o1-dev"),
        ];
        assert_eq!(
            validate_workflows(&workflows),
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
            validate_workflows(&[workflow]),
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
            validate_workflows(&[workflow]),
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
            validate_workflows(&[workflow]),
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
            validate_workflows(&workflows),
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
            validate_workflows(&workflows),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "model asset ID, name, install path, or download source is invalid"
                    .to_string()
            })
        );
    }
}
