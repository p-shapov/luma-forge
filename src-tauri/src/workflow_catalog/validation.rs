use std::collections::HashSet;

use crate::domain::{
    runtime_contract::RuntimeCatalog,
    workflow_preset::{ModelAssetSource, WorkflowPreset},
};

use super::errors::WorkflowCatalogError;

pub(super) fn validate_runtime_catalog(
    catalog: &RuntimeCatalog,
) -> Result<(), WorkflowCatalogError> {
    if catalog.contracts.is_empty() {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    let mut contract_ids = HashSet::new();
    for contract in &catalog.contracts {
        if contract.id.trim().is_empty() || !contract_ids.insert(contract.id.as_str()) {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        if contract.revisions.is_empty() {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        let mut revision_versions = HashSet::new();
        for revision in &contract.revisions {
            if revision.version.trim().is_empty()
                || !revision_versions.insert(revision.version.as_str())
                || revision.image_ref.trim().is_empty()
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }
    }

    Ok(())
}

pub(super) fn validate_workflows(
    workflows: &[WorkflowPreset],
    endpoint_contract_catalog: &RuntimeCatalog,
    provisioner_contract_catalog: &RuntimeCatalog,
) -> Result<(), WorkflowCatalogError> {
    if workflows.is_empty() {
        return Err(WorkflowCatalogError::ValidationFailed);
    }

    let mut workflow_ids = HashSet::new();
    for workflow in workflows {
        if workflow.id.trim().is_empty()
            || !workflow_ids.insert(workflow.id.as_str())
            || workflow.version.trim().is_empty()
            || workflow.name.trim().is_empty()
        {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        let remote_requirements = &workflow.remote_runtime_requirements;
        if remote_requirements.required_base_volume_size_bytes == 0
            || remote_requirements.provider_requirements.is_empty()
        {
            return Err(WorkflowCatalogError::ValidationFailed);
        }

        for provider_requirements in &remote_requirements.provider_requirements {
            if endpoint_contract_catalog
                .resolve(&provider_requirements.endpoint_contract)
                .is_none()
                || provisioner_contract_catalog
                    .resolve(&provider_requirements.provisioner_contract)
                    .is_none()
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }

        for asset in &workflow.required_model_assets {
            let install_path = asset.install_comfyui_relative_path.trim();
            if asset.id.trim().is_empty()
                || asset.name.trim().is_empty()
                || install_path.is_empty()
                || install_path.starts_with('/')
                || install_path.starts_with('\\')
                || install_path.contains('\\')
                || !install_path
                    .split('/')
                    .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
                || !is_valid_model_asset_source(&asset.download_source)
            {
                return Err(WorkflowCatalogError::ValidationFailed);
            }
        }
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
            let file_path = file_path.trim();
            is_valid_hugging_face_repository_id(repository_id)
                && !file_path.is_empty()
                && !file_path.starts_with('/')
                && !file_path.starts_with('\\')
                && !file_path.contains('\\')
                && file_path
                    .split('/')
                    .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provider::GpuCloudProviderId,
        runtime_contract::{RuntimeContract, RuntimeContractReference, RuntimeContractRevision},
        workflow_preset::{
            ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
            RemoteRuntimeRequirements, WorkflowExecutionType,
        },
    };

    fn runtime_catalog(id: &str, version: &str) -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: id.to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: version.to_string(),
                    image_ref: "ghcr.io/example/image@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                }],
            }],
        }
    }

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

    fn valid_workflow(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI HiDream O1 Dev".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: true,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 18837849239,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "comfyui-hidream-o1-dev".to_string(),
                        version: "1.0.15".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "luma-forge-provisioner".to_string(),
                        version: "1.0.6".to_string(),
                    },
                }],
            },
            required_model_assets: vec![valid_asset()],
        }
    }

    #[test]
    fn validate_runtime_catalog_accepts_valid_catalog() {
        assert_eq!(
            validate_runtime_catalog(&runtime_catalog("comfyui-hidream-o1-dev", "1.0.15")),
            Ok(())
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_empty_catalog() {
        assert_eq!(
            validate_runtime_catalog(&RuntimeCatalog { contracts: vec![] }),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_duplicate_contract_ids() {
        let catalog = RuntimeCatalog {
            contracts: vec![
                RuntimeContract {
                    id: "duplicate".to_string(),
                    revisions: vec![RuntimeContractRevision {
                        version: "1.0.0".to_string(),
                        image_ref: "image-a".to_string(),
                    }],
                },
                RuntimeContract {
                    id: "duplicate".to_string(),
                    revisions: vec![RuntimeContractRevision {
                        version: "1.0.1".to_string(),
                        image_ref: "image-b".to_string(),
                    }],
                },
            ],
        };
        assert_eq!(
            validate_runtime_catalog(&catalog),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_accepts_valid_workflow() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];
        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6")
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_workflows_rejects_duplicate_workflow_ids() {
        let workflows = vec![
            valid_workflow("comfyui-hidream-o1-dev"),
            valid_workflow("comfyui-hidream-o1-dev"),
        ];
        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6")
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_missing_endpoint_contract_reference() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];
        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("different-endpoint", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6")
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_missing_provisioner_contract_reference() {
        let workflows = vec![valid_workflow("comfyui-hidream-o1-dev")];
        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("different-provisioner", "1.0.6")
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }

    #[test]
    fn validate_workflows_rejects_invalid_model_asset_paths() {
        let mut workflow = valid_workflow("comfyui-hidream-o1-dev");
        workflow.required_model_assets[0].install_comfyui_relative_path =
            "../outside.safetensors".to_string();
        let workflows = vec![workflow];
        assert_eq!(
            validate_workflows(
                &workflows,
                &runtime_catalog("comfyui-hidream-o1-dev", "1.0.15"),
                &runtime_catalog("luma-forge-provisioner", "1.0.6")
            ),
            Err(WorkflowCatalogError::ValidationFailed)
        );
    }
}
