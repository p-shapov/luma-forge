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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::runtime::{RuntimeCatalog, RuntimeContract, RuntimeContractRevision};

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn image_ref(name: &str, digest: &str) -> String {
        format!("ghcr.io/luma-forge/{name}@sha256:{digest}")
    }

    fn valid_revision(version: &str) -> RuntimeContractRevision {
        RuntimeContractRevision {
            version: version.to_string(),
            endpoint_image_ref: image_ref("endpoint", DIGEST_B),
        }
    }

    fn valid_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                revisions: vec![valid_revision("1.0.0")],
            }],
        }
    }

    #[test]
    fn validate_runtime_catalog_accepts_valid_catalog() {
        assert_eq!(validate_runtime_catalog(&valid_catalog()), Ok(()));
    }

    #[test]
    fn validate_runtime_catalog_rejects_invalid_contract_shapes() {
        let invalid_catalogs = [
            RuntimeCatalog { contracts: vec![] },
            RuntimeCatalog {
                contracts: vec![RuntimeContract {
                    id: "ComfyUI".to_string(),
                    revisions: vec![valid_revision("1.0.0")],
                }],
            },
            RuntimeCatalog {
                contracts: vec![
                    RuntimeContract {
                        id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                        revisions: vec![valid_revision("1.0.0")],
                    },
                    RuntimeContract {
                        id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                        revisions: vec![valid_revision("1.0.1")],
                    },
                ],
            },
            RuntimeCatalog {
                contracts: vec![RuntimeContract {
                    id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                    revisions: vec![],
                }],
            },
        ];

        for catalog in invalid_catalogs {
            assert_eq!(
                validate_runtime_catalog(&catalog),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_runtime_catalog_rejects_invalid_revision_shapes() {
        let invalid_revisions = [
            RuntimeContractRevision {
                version: "01.0.0".to_string(),
                ..valid_revision("1.0.0")
            },
            RuntimeContractRevision {
                version: "1.0".to_string(),
                ..valid_revision("1.0.0")
            },
            RuntimeContractRevision {
                endpoint_image_ref: image_ref(
                    "endpoint",
                    "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                ),
                ..valid_revision("1.0.0")
            },
        ];

        for revision in invalid_revisions {
            let catalog = RuntimeCatalog {
                contracts: vec![RuntimeContract {
                    id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                    revisions: vec![revision],
                }],
            };

            assert_eq!(
                validate_runtime_catalog(&catalog),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_runtime_contract_reference_requires_existing_contract_revision() {
        let catalog = valid_catalog();

        assert_eq!(
            validate_runtime_contract_reference(
                "comfyui-hidream-o1-dev-python312-cu121",
                "1.0.0",
                &catalog
            ),
            Ok(())
        );
        assert_eq!(
            validate_runtime_contract_reference(
                "comfyui-hidream-o1-dev-python312-cu121",
                "2.0.0",
                &catalog
            ),
            Err(DomainValidationError)
        );
        assert_eq!(
            validate_runtime_contract_reference("ComfyUI", "1.0.0", &catalog),
            Err(DomainValidationError)
        );
    }

    #[test]
    fn validate_resolved_runtime_snapshot_reuses_runtime_shape_rules() {
        let snapshot = valid_catalog()
            .resolve("comfyui-hidream-o1-dev-python312-cu121", "1.0.0")
            .expect("valid runtime should resolve");

        assert_eq!(validate_resolved_runtime_snapshot(&snapshot), Ok(()));

        let invalid_snapshot = ResolvedRuntimeImageSnapshot {
            contract_version: "1.0".to_string(),
            ..snapshot
        };

        assert_eq!(
            validate_resolved_runtime_snapshot(&invalid_snapshot),
            Err(DomainValidationError)
        );
    }
}
