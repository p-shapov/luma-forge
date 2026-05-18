use std::collections::HashSet;

use crate::domain::error::{DomainValidationError, DomainValidationResult};

use super::{ResolvedRuntimeImageSnapshot, RuntimeCatalog};

pub fn validate_runtime_catalog(catalog: &RuntimeCatalog) -> DomainValidationResult {
    if catalog.contracts.is_empty() {
        return Err(DomainValidationError);
    }

    let mut contract_ids = HashSet::new();
    for contract in &catalog.contracts {
        if !is_contract_id(&contract.id)
            || !contract_ids.insert(contract.id.as_str())
            || contract.revisions.is_empty()
        {
            return Err(DomainValidationError);
        }

        let mut versions = HashSet::new();
        for revision in &contract.revisions {
            if !is_semver(&revision.version)
                || !versions.insert(revision.version.as_str())
                || !is_immutable_image_ref(&revision.provisioner_image_ref)
                || !is_immutable_image_ref(&revision.endpoint_image_ref)
            {
                return Err(DomainValidationError);
            }
        }
    }

    Ok(())
}

pub fn validate_runtime_contract_reference(
    contract_id: &str,
    contract_version: &str,
    catalog: &RuntimeCatalog,
) -> DomainValidationResult {
    if is_contract_id(contract_id)
        && is_semver(contract_version)
        && catalog.resolve(contract_id, contract_version).is_some()
    {
        Ok(())
    } else {
        Err(DomainValidationError)
    }
}

pub fn validate_resolved_runtime_snapshot(
    snapshot: &ResolvedRuntimeImageSnapshot,
) -> DomainValidationResult {
    if !is_contract_id(&snapshot.contract_id)
        || !is_semver(&snapshot.contract_version)
        || !is_immutable_image_ref(&snapshot.provisioner_image_ref)
        || !is_immutable_image_ref(&snapshot.endpoint_image_ref)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn is_contract_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
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
