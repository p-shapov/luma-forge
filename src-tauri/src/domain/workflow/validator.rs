use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    runtime::{validator as runtime_validator, RuntimeCatalog},
    validation::{is_blank, is_safe_relative_path},
};

use super::{CustomNodeGitSource, ModelAssetSource, WorkflowCatalog};

pub fn validate_workflow_catalog(
    catalog: &WorkflowCatalog,
    runtime_catalog: &RuntimeCatalog,
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

        for node in &preset.required_custom_nodes {
            if is_blank(&node.id)
                || is_blank(&node.name)
                || !is_valid_custom_node_source(&node.git_source)
                || !is_safe_custom_node_path(&node.install.comfyui_custom_nodes_relative_path)
                || !is_optional_safe_relative_path(&node.install.python_requirements_path)
            {
                return Err(DomainValidationError);
            }
        }
    }

    Ok(())
}

fn is_valid_custom_node_source(source: &CustomNodeGitSource) -> bool {
    match source {
        CustomNodeGitSource::Git {
            repository_url,
            revision,
        } => is_url_shaped(repository_url) && is_immutable_git_revision(revision),
    }
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

fn is_safe_custom_node_path(value: &str) -> bool {
    if !is_safe_relative_path(value) {
        return false;
    }

    let mut segments = value.trim().split(['/', '\\']);
    matches!(segments.next(), Some("custom_nodes")) && segments.next().is_some()
}

fn is_optional_safe_relative_path(value: &Option<String>) -> bool {
    value.as_deref().map(is_safe_relative_path).unwrap_or(true)
}

fn is_url_shaped(value: &str) -> bool {
    let value = value.trim();
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };

    !scheme.is_empty()
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
        && !rest.is_empty()
        && !rest.chars().any(char::is_whitespace)
        && !rest.starts_with('/')
}

fn is_immutable_git_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
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
        runtime::{RuntimeCatalog, RuntimeContract, RuntimeContractRevision},
        workflow::{
            CustomNode, CustomNodeInstall, ModelAsset, ModelAssetInstall, ModelAssetKind,
            RuntimeContractReference, WorkflowExecutionType, WorkflowPreset,
        },
    };

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn runtime_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: "comfyui-python312-cu121".to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: "1.0.0".to_string(),
                    provisioner_image_ref: format!(
                        "ghcr.io/luma-forge/provisioner@sha256:{DIGEST_A}"
                    ),
                    endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
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

    fn valid_custom_node() -> CustomNode {
        CustomNode {
            id: "example-node".to_string(),
            name: "Example Node".to_string(),
            git_source: CustomNodeGitSource::Git {
                repository_url: "https://github.com/example/node.git".to_string(),
                revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
            },
            install: CustomNodeInstall {
                comfyui_custom_nodes_relative_path: "custom_nodes/example-node".to_string(),
                python_requirements_path: Some(
                    "custom_nodes/example-node/requirements.txt".to_string(),
                ),
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
            required_model_assets: vec![valid_model_asset()],
            required_custom_nodes: vec![valid_custom_node()],
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
            validate_workflow_catalog(&valid_catalog(), &runtime_catalog()),
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
                validate_workflow_catalog(&catalog, &runtime_catalog()),
                Err(DomainValidationError)
            );
        }
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
                validate_workflow_catalog(&catalog, &runtime_catalog()),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_workflow_catalog_rejects_mutable_or_unsafe_custom_nodes() {
        let invalid_nodes = [
            CustomNode {
                id: " ".to_string(),
                ..valid_custom_node()
            },
            CustomNode {
                git_source: CustomNodeGitSource::Git {
                    repository_url: "github.com/example/node.git".to_string(),
                    revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                },
                ..valid_custom_node()
            },
            CustomNode {
                git_source: CustomNodeGitSource::Git {
                    repository_url: "https://github.com/example/node.git".to_string(),
                    revision: "main".to_string(),
                },
                ..valid_custom_node()
            },
            CustomNode {
                install: CustomNodeInstall {
                    comfyui_custom_nodes_relative_path: "nodes/example".to_string(),
                    python_requirements_path: None,
                },
                ..valid_custom_node()
            },
            CustomNode {
                install: CustomNodeInstall {
                    comfyui_custom_nodes_relative_path: "custom_nodes/example".to_string(),
                    python_requirements_path: Some("../requirements.txt".to_string()),
                },
                ..valid_custom_node()
            },
        ];

        for node in invalid_nodes {
            let catalog = WorkflowCatalog {
                workflow_presets: vec![WorkflowPreset {
                    required_custom_nodes: vec![node],
                    ..valid_preset("comfyui-t2i-basic")
                }],
            };

            assert_eq!(
                validate_workflow_catalog(&catalog, &runtime_catalog()),
                Err(DomainValidationError)
            );
        }
    }
}
