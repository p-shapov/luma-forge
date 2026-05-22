use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    validation::is_safe_absolute_posix_path,
};

use super::{ProvisionerCatalog, ResolvedProvisionerImageSnapshot};

pub fn validate_provisioner_catalog(catalog: &ProvisionerCatalog) -> DomainValidationResult {
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
                || !is_immutable_image_ref(&revision.provisioner_worker_image_ref)
                || !is_safe_absolute_posix_path(&revision.volume_mount_path)
            {
                return Err(DomainValidationError);
            }
        }
    }

    Ok(())
}

pub fn validate_provisioner_contract_reference(
    contract_id: &str,
    contract_version: &str,
    catalog: &ProvisionerCatalog,
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

pub fn validate_resolved_provisioner_snapshot(
    snapshot: &ResolvedProvisionerImageSnapshot,
) -> DomainValidationResult {
    if !is_contract_id(&snapshot.contract_id)
        || !is_semver(&snapshot.contract_version)
        || !is_immutable_image_ref(&snapshot.provisioner_worker_image_ref)
        || !is_safe_absolute_posix_path(&snapshot.volume_mount_path)
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
    use crate::domain::provisioner::{
        ProvisionerCatalog, ProvisionerContract, ProvisionerContractRevision,
    };

    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn image_ref(name: &str, digest: &str) -> String {
        format!("ghcr.io/luma-forge/{name}@sha256:{digest}")
    }

    fn valid_revision(version: &str) -> ProvisionerContractRevision {
        ProvisionerContractRevision {
            version: version.to_string(),
            provisioner_worker_image_ref: image_ref("provisioner", DIGEST_C),
            volume_mount_path: "/workspace".to_string(),
        }
    }

    fn valid_catalog() -> ProvisionerCatalog {
        ProvisionerCatalog {
            contracts: vec![ProvisionerContract {
                id: "luma-forge-provisioner".to_string(),
                revisions: vec![valid_revision("1.0.0")],
            }],
        }
    }

    #[test]
    fn validate_provisioner_catalog_accepts_valid_catalog() {
        assert_eq!(validate_provisioner_catalog(&valid_catalog()), Ok(()));
    }

    #[test]
    fn validate_provisioner_catalog_rejects_invalid_contract_shapes() {
        let invalid_catalogs = [
            ProvisionerCatalog { contracts: vec![] },
            ProvisionerCatalog {
                contracts: vec![ProvisionerContract {
                    id: "Provisioner".to_string(),
                    revisions: vec![valid_revision("1.0.0")],
                }],
            },
            ProvisionerCatalog {
                contracts: vec![
                    ProvisionerContract {
                        id: "luma-forge-provisioner".to_string(),
                        revisions: vec![valid_revision("1.0.0")],
                    },
                    ProvisionerContract {
                        id: "luma-forge-provisioner".to_string(),
                        revisions: vec![valid_revision("1.0.1")],
                    },
                ],
            },
            ProvisionerCatalog {
                contracts: vec![ProvisionerContract {
                    id: "luma-forge-provisioner".to_string(),
                    revisions: vec![],
                }],
            },
        ];

        for catalog in invalid_catalogs {
            assert_eq!(
                validate_provisioner_catalog(&catalog),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_provisioner_catalog_rejects_invalid_revision_shapes() {
        let invalid_revisions = [
            ProvisionerContractRevision {
                version: "01.0.0".to_string(),
                ..valid_revision("1.0.0")
            },
            ProvisionerContractRevision {
                version: "1.0".to_string(),
                ..valid_revision("1.0.0")
            },
            ProvisionerContractRevision {
                provisioner_worker_image_ref: image_ref(
                    "provisioner",
                    "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                ),
                ..valid_revision("1.0.0")
            },
            ProvisionerContractRevision {
                volume_mount_path: "../workspace".to_string(),
                ..valid_revision("1.0.0")
            },
        ];

        for revision in invalid_revisions {
            let catalog = ProvisionerCatalog {
                contracts: vec![ProvisionerContract {
                    id: "luma-forge-provisioner".to_string(),
                    revisions: vec![revision],
                }],
            };

            assert_eq!(
                validate_provisioner_catalog(&catalog),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_provisioner_contract_reference_requires_existing_contract_revision() {
        let catalog = valid_catalog();

        assert_eq!(
            validate_provisioner_contract_reference("luma-forge-provisioner", "1.0.0", &catalog),
            Ok(())
        );
        assert_eq!(
            validate_provisioner_contract_reference("luma-forge-provisioner", "2.0.0", &catalog),
            Err(DomainValidationError)
        );
        assert_eq!(
            validate_provisioner_contract_reference("Provisioner", "1.0.0", &catalog),
            Err(DomainValidationError)
        );
    }

    #[test]
    fn validate_resolved_provisioner_snapshot_reuses_provisioner_shape_rules() {
        let snapshot = valid_catalog()
            .resolve("luma-forge-provisioner", "1.0.0")
            .expect("valid provisioner should resolve");

        assert_eq!(validate_resolved_provisioner_snapshot(&snapshot), Ok(()));

        let invalid_snapshot = ResolvedProvisionerImageSnapshot {
            volume_mount_path: "workspace".to_string(),
            ..snapshot
        };

        assert_eq!(
            validate_resolved_provisioner_snapshot(&invalid_snapshot),
            Err(DomainValidationError)
        );
    }
}
