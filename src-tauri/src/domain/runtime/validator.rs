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
            || contract
                .runtime_metadata
                .base_dependency_record_paths
                .iter()
                .any(|path| !is_safe_relative_path(path))
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
                || !is_safe_absolute_posix_path(
                    &implementation
                        .image_metadata
                        .provisioner_runtime_archive_path,
                )
                || !is_safe_absolute_posix_path(
                    &implementation
                        .image_metadata
                        .provisioner_runtime_metadata_path,
                )
                || !is_safe_absolute_posix_path(
                    &implementation.image_metadata.endpoint_runtime_contract_path,
                )
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
    {
        return Err(DomainValidationError);
    }

    Ok(())
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
