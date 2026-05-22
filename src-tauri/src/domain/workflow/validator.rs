use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    provisioner::{validator as provisioner_validator, ProvisionerCatalog},
    runtime::{validator as runtime_validator, RuntimeCatalog},
    validation::{is_blank, is_safe_relative_path},
};

use super::{ModelAssetSource, WorkflowCatalog};

pub fn validate_workflow_catalog(
    catalog: &WorkflowCatalog,
    runtime_catalog: &RuntimeCatalog,
    provisioner_catalog: &ProvisionerCatalog,
) -> DomainValidationResult {
    if catalog.workflow_presets.is_empty() {
        return Err(DomainValidationError);
    }

    let mut ids = HashSet::new();
    for preset in &catalog.workflow_presets {
        if is_blank(&preset.id)
            || is_blank(&preset.version)
            || is_blank(&preset.name)
            || preset.required_base_volume_size_bytes == 0
            || !ids.insert(preset.id.as_str())
        {
            return Err(DomainValidationError);
        }

        for asset in &preset.required_model_assets {
            if is_blank(&asset.id)
                || is_blank(&asset.name)
                || !is_valid_model_asset_source(&asset.download_source)
                || !is_safe_relative_path(&asset.install.comfyui_relative_path)
            {
                return Err(DomainValidationError);
            }
        }

        if runtime_validator::validate_runtime_contract_reference(
            &preset.runtime_contract.id,
            &preset.runtime_contract.version,
            runtime_catalog,
        )
        .is_err()
        {
            return Err(DomainValidationError);
        }

        if provisioner_validator::validate_provisioner_contract_reference(
            &preset.provisioner_contract.id,
            &preset.provisioner_contract.version,
            provisioner_catalog,
        )
        .is_err()
        {
            return Err(DomainValidationError);
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
            is_huggingface_repository_id(repository_id)
                && is_safe_relative_path(file_path)
                && !is_blank(revision)
        }
    }
}

fn is_huggingface_repository_id(value: &str) -> bool {
    let value = value.trim();
    let segments: Vec<_> = value.split('/').collect();
    segments.len() == 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && *segment != "."
                && *segment != ".."
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provisioner::{ProvisionerCatalog, ProvisionerContract, ProvisionerContractRevision},
        runtime::{RuntimeCatalog, RuntimeContract, RuntimeContractRevision},
        workflow::{
            ModelAsset, ModelAssetInstall, ModelAssetKind, ProvisionerContractReference,
            RuntimeContractReference, WorkflowExecutionType, WorkflowPreset,
        },
    };

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn runtime_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: "comfyui-python312-cu121".to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: "1.0.0".to_string(),
                    endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
                }],
            }],
        }
    }

    fn provisioner_catalog() -> ProvisionerCatalog {
        ProvisionerCatalog {
            contracts: vec![ProvisionerContract {
                id: "luma-forge-provisioner".to_string(),
                revisions: vec![ProvisionerContractRevision {
                    version: "1.0.0".to_string(),
                    provisioner_worker_image_ref: format!(
                        "ghcr.io/luma-forge/provisioner@sha256:{DIGEST_C}"
                    ),
                    volume_mount_path: "/workspace".to_string(),
                }],
            }],
        }
    }

    fn valid_model_asset() -> ModelAsset {
        ModelAsset {
            id: "sdxl-base".to_string(),
            name: "SDXL Base".to_string(),
            model_asset_kind: ModelAssetKind::Checkpoint,
            download_source: ModelAssetSource::Huggingface {
                repository_id: "stabilityai/stable-diffusion-xl-base-1.0".to_string(),
                file_path: "sd_xl_base_1.0.safetensors".to_string(),
                revision: "462165984030d82259a11f4367a4eed129e94a7b".to_string(),
            },
            install: ModelAssetInstall {
                comfyui_relative_path: "models/checkpoints/sd_xl_base_1.0.safetensors".to_string(),
            },
        }
    }

    fn valid_preset(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI Text to Image".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 80 * 1024 * 1024 * 1024,
            runtime_contract: RuntimeContractReference {
                id: "comfyui-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: ProvisionerContractReference {
                id: "luma-forge-provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![valid_model_asset()],
        }
    }

    fn valid_catalog() -> WorkflowCatalog {
        WorkflowCatalog {
            workflow_presets: vec![valid_preset("comfyui-t2i-basic")],
        }
    }

    #[test]
    fn validate_workflow_catalog_accepts_valid_catalog() {
        assert_eq!(
            validate_workflow_catalog(&valid_catalog(), &runtime_catalog(), &provisioner_catalog()),
            Ok(())
        );
    }

    #[test]
    fn validate_workflow_catalog_rejects_invalid_preset_metadata() {
        let invalid_catalogs = [
            WorkflowCatalog {
                workflow_presets: vec![],
            },
            WorkflowCatalog {
                workflow_presets: vec![WorkflowPreset {
                    id: " ".to_string(),
                    ..valid_preset("comfyui-t2i-basic")
                }],
            },
            WorkflowCatalog {
                workflow_presets: vec![WorkflowPreset {
                    required_base_volume_size_bytes: 0,
                    ..valid_preset("comfyui-t2i-basic")
                }],
            },
            WorkflowCatalog {
                workflow_presets: vec![
                    valid_preset("comfyui-t2i-basic"),
                    valid_preset("comfyui-t2i-basic"),
                ],
            },
            WorkflowCatalog {
                workflow_presets: vec![WorkflowPreset {
                    runtime_contract: RuntimeContractReference {
                        id: "comfyui-python312-cu121".to_string(),
                        version: "2.0.0".to_string(),
                    },
                    ..valid_preset("comfyui-t2i-basic")
                }],
            },
        ];

        for catalog in invalid_catalogs {
            assert_eq!(
                validate_workflow_catalog(&catalog, &runtime_catalog(), &provisioner_catalog()),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_workflow_catalog_rejects_stale_provisioner_contract_reference() {
        let catalog = WorkflowCatalog {
            workflow_presets: vec![WorkflowPreset {
                provisioner_contract: ProvisionerContractReference {
                    id: "luma-forge-provisioner".to_string(),
                    version: "2.0.0".to_string(),
                },
                ..valid_preset("comfyui-t2i-basic")
            }],
        };

        assert_eq!(
            validate_workflow_catalog(&catalog, &runtime_catalog(), &provisioner_catalog()),
            Err(DomainValidationError)
        );
    }

    #[test]
    fn validate_workflow_catalog_rejects_unsafe_model_assets() {
        let invalid_assets = [
            ModelAsset {
                id: " ".to_string(),
                ..valid_model_asset()
            },
            ModelAsset {
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "../stable-diffusion".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                ..valid_model_asset()
            },
            ModelAsset {
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/repo".to_string(),
                    file_path: "../model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                ..valid_model_asset()
            },
            ModelAsset {
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/repo".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: " ".to_string(),
                },
                ..valid_model_asset()
            },
            ModelAsset {
                install: ModelAssetInstall {
                    comfyui_relative_path: "/models/checkpoints/model.safetensors".to_string(),
                },
                ..valid_model_asset()
            },
        ];

        for asset in invalid_assets {
            let catalog = WorkflowCatalog {
                workflow_presets: vec![WorkflowPreset {
                    required_model_assets: vec![asset],
                    ..valid_preset("comfyui-t2i-basic")
                }],
            };

            assert_eq!(
                validate_workflow_catalog(&catalog, &runtime_catalog(), &provisioner_catalog()),
                Err(DomainValidationError)
            );
        }
    }
}
