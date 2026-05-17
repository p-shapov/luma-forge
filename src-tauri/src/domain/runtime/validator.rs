use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    validation::{is_blank, is_safe_absolute_posix_path, is_safe_relative_path},
};

use super::{ResolvedRuntimeImplementationSnapshot, RuntimeCatalog, RuntimeContractReference};

pub fn validate_runtime_catalog(catalog: &RuntimeCatalog) -> DomainValidationResult {
    if is_blank(&catalog.id) || is_blank(&catalog.version) || catalog.runtime_contracts.is_empty() {
        return Err(DomainValidationError);
    }

    let mut contract_keys = HashSet::new();
    for contract in &catalog.runtime_contracts {
        if !is_valid_contract_reference(&RuntimeContractReference {
            id: contract.id.clone(),
            version: contract.version.clone(),
        }) || is_blank(&contract.display_name)
            || contract.implementation_revisions.is_empty()
            || !contract_keys.insert((contract.id.as_str(), contract.version.as_str()))
        {
            return Err(DomainValidationError);
        }

        if is_blank(&contract.runtime_metadata.environment_kind)
            || is_blank(&contract.runtime_metadata.python_version)
            || is_blank(&contract.runtime_metadata.platform)
            || !is_immutable_git_revision(&contract.runtime_metadata.comfyui_revision)
            || !is_valid_manifest_compatibility(
                &contract.runtime_metadata.runtime_manifest_compatibility,
            )
            || !is_valid_overlay_policy(&contract.runtime_metadata.workspace_overlay_policy)
        {
            return Err(DomainValidationError);
        }

        let mut revisions = HashSet::new();
        let mut has_default = false;
        for implementation in &contract.implementation_revisions {
            if is_blank(&implementation.revision)
                || !revisions.insert(implementation.revision.as_str())
                || !is_immutable_image_ref(&implementation.provisioner_image_ref)
                || !is_immutable_image_ref(&implementation.endpoint_image_ref)
                || !is_valid_image_metadata_paths(&implementation.image_metadata)
                || implementation
                    .image_metadata
                    .image_base_dependency_record_paths
                    .is_empty()
                || implementation
                    .image_metadata
                    .image_base_dependency_record_paths
                    .iter()
                    .any(|path| !is_safe_relative_path(path))
            {
                return Err(DomainValidationError);
            }

            if implementation.revision == contract.default_implementation_revision {
                has_default = true;
            }
        }

        if !has_default {
            return Err(DomainValidationError);
        }
    }

    Ok(())
}

pub fn validate_runtime_contract_reference(
    reference: &RuntimeContractReference,
    catalog: &RuntimeCatalog,
) -> DomainValidationResult {
    if is_valid_contract_reference(reference) && catalog.resolve_default(reference).is_some() {
        Ok(())
    } else {
        Err(DomainValidationError)
    }
}

pub fn validate_resolved_runtime_snapshot(
    snapshot: &ResolvedRuntimeImplementationSnapshot,
) -> DomainValidationResult {
    if !is_valid_contract_reference(&RuntimeContractReference {
        id: snapshot.contract_id.clone(),
        version: snapshot.contract_version.clone(),
    }) || is_blank(&snapshot.implementation_revision)
        || !is_immutable_image_ref(&snapshot.provisioner_image_ref)
        || !is_immutable_image_ref(&snapshot.endpoint_image_ref)
        || is_blank(&snapshot.runtime_metadata.environment_kind)
        || is_blank(&snapshot.runtime_metadata.python_version)
        || is_blank(&snapshot.runtime_metadata.platform)
        || !is_immutable_git_revision(&snapshot.runtime_metadata.comfyui_revision)
        || !is_valid_manifest_compatibility(
            &snapshot.runtime_metadata.runtime_manifest_compatibility,
        )
        || !is_valid_overlay_policy(&snapshot.runtime_metadata.workspace_overlay_policy)
        || !is_valid_image_metadata_paths(&snapshot.image_metadata)
        || snapshot
            .image_metadata
            .image_base_dependency_record_paths
            .is_empty()
        || snapshot
            .image_metadata
            .image_base_dependency_record_paths
            .iter()
            .any(|path| !is_safe_relative_path(path))
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn is_valid_manifest_compatibility(compatibility: &super::RuntimeManifestCompatibility) -> bool {
    !is_blank(&compatibility.manifest_version)
}

fn is_valid_image_metadata_paths(metadata: &super::RuntimeImageMetadata) -> bool {
    is_safe_absolute_posix_path(&metadata.image_runtime_root_path)
        && is_image_runtime_child_path(
            &metadata.image_runtime_root_path,
            &metadata.image_python_interpreter_path,
        )
        && is_image_runtime_child_path(
            &metadata.image_runtime_root_path,
            &metadata.image_comfyui_root_path,
        )
        && is_image_runtime_child_path(
            &metadata.image_runtime_root_path,
            &metadata.provisioner_runtime_metadata_path,
        )
        && is_image_runtime_child_path(
            &metadata.image_runtime_root_path,
            &metadata.endpoint_runtime_contract_path,
        )
}

fn is_image_runtime_child_path(root: &str, path: &str) -> bool {
    is_safe_absolute_posix_path(path)
        && path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/') && suffix.len() > 1)
}

fn is_valid_overlay_policy(policy: &super::WorkspaceOverlayPolicy) -> bool {
    is_safe_relative_path(&policy.python_overlay_path)
        && policy.import_path_precedence == "overlay_first"
        && !policy.protected_package_names.is_empty()
        && policy
            .protected_package_names
            .iter()
            .all(|name| is_python_distribution_name(name))
        && policy
            .protected_package_prefixes
            .iter()
            .all(|prefix| is_python_distribution_prefix(prefix))
}

fn is_python_distribution_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
                || character == '.'
        })
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

fn is_python_distribution_prefix(value: &str) -> bool {
    value.ends_with('-') && is_python_distribution_name(&value[..value.len() - 1])
}

fn is_valid_contract_reference(reference: &RuntimeContractReference) -> bool {
    is_stable_identifier(&reference.id) && is_semver(&reference.version)
}

fn is_stable_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
}

fn is_semver(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|character| character.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        })
}

fn is_immutable_image_ref(value: &str) -> bool {
    let Some((name, digest)) = value.split_once("@sha256:") else {
        return false;
    };
    !name.trim().is_empty()
        && digest.len() == 64
        && digest
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn is_immutable_git_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::runtime::{
        RuntimeContract, RuntimeImageMetadata, RuntimeImplementationRevision,
        RuntimeManifestCompatibility, RuntimeMetadata, WorkspaceOverlayPolicy,
    };

    #[test]
    fn rejects_runtime_catalog_with_unsupported_import_path_precedence() {
        let mut catalog = valid_runtime_catalog();
        catalog.runtime_contracts[0]
            .runtime_metadata
            .workspace_overlay_policy
            .import_path_precedence = "base_first".to_string();

        validate_runtime_catalog(&catalog).expect_err("unsupported precedence should fail");
    }

    #[test]
    fn rejects_resolved_runtime_snapshot_with_unsupported_import_path_precedence() {
        let mut snapshot = valid_runtime_catalog()
            .resolve_default(&RuntimeContractReference {
                id: "comfyui-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            })
            .expect("default runtime");
        snapshot
            .runtime_metadata
            .workspace_overlay_policy
            .import_path_precedence = "base_first".to_string();

        validate_resolved_runtime_snapshot(&snapshot)
            .expect_err("unsupported precedence should fail");
    }

    fn valid_runtime_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            id: "catalog".to_string(),
            version: "1".to_string(),
            runtime_contracts: vec![RuntimeContract {
                id: "comfyui-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
                display_name: "Runtime".to_string(),
                runtime_metadata: RuntimeMetadata {
                    environment_kind: "image_baked_comfyui_runtime".to_string(),
                    python_version: "3.12".to_string(),
                    platform: "linux-x86_64-cuda".to_string(),
                    comfyui_revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
                    runtime_manifest_compatibility: RuntimeManifestCompatibility {
                        manifest_version: "1".to_string(),
                    },
                    workspace_overlay_policy: WorkspaceOverlayPolicy {
                        python_overlay_path: ".luma-forge/python-overlay".to_string(),
                        import_path_precedence: "overlay_first".to_string(),
                        protected_package_names: vec!["torch".to_string()],
                        protected_package_prefixes: vec!["nvidia-".to_string()],
                    },
                },
                implementation_revisions: vec![RuntimeImplementationRevision {
                    revision: "2026.05.16-004".to_string(),
                    provisioner_image_ref: format!(
                        "ghcr.io/luma-forge/provisioner-worker@sha256:{}",
                        "1".repeat(64)
                    ),
                    endpoint_image_ref: format!(
                        "ghcr.io/luma-forge/endpoint-worker@sha256:{}",
                        "2".repeat(64)
                    ),
                    image_metadata: RuntimeImageMetadata {
                        image_runtime_root_path: "/opt/luma-forge/runtime".to_string(),
                        image_python_interpreter_path: "/opt/luma-forge/runtime/.venv/bin/python"
                            .to_string(),
                        image_comfyui_root_path: "/opt/luma-forge/runtime/ComfyUI".to_string(),
                        image_base_dependency_record_paths: vec![
                            "base-runtime/pip-freeze.txt".to_string()
                        ],
                        provisioner_runtime_metadata_path:
                            "/opt/luma-forge/runtime/runtime-metadata.json".to_string(),
                        endpoint_runtime_contract_path:
                            "/opt/luma-forge/runtime/runtime-contract.json".to_string(),
                    },
                }],
                default_implementation_revision: "2026.05.16-004".to_string(),
            }],
        }
    }
}
