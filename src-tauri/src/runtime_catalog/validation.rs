use std::collections::HashSet;

use crate::domain::runtime_contract::{RuntimeCatalog, RuntimeContract, RuntimeContractRevision};

use super::RuntimeCatalogError;

const EMPTY_RUNTIME_CATALOG: &str = "catalog is empty";
const INVALID_CONTRACT_ID: &str = "contract ID is empty or duplicate";
const EMPTY_CONTRACT_REVISIONS: &str = "contract has no revisions";
const INVALID_CONTRACT_REVISION: &str =
    "revision version is empty, duplicate, or image reference is empty";

pub(super) fn validate_runtime_catalog(
    catalog: &RuntimeCatalog,
) -> Result<(), RuntimeCatalogError> {
    if catalog.contracts.is_empty() {
        return validation_error(EMPTY_RUNTIME_CATALOG);
    }

    let mut contract_ids = HashSet::new();
    for contract in &catalog.contracts {
        validate_runtime_contract(contract, &mut contract_ids)?;
    }

    Ok(())
}

fn validate_runtime_contract<'catalog>(
    contract: &'catalog RuntimeContract,
    contract_ids: &mut HashSet<&'catalog str>,
) -> Result<(), RuntimeCatalogError> {
    if contract.id.trim().is_empty() || !contract_ids.insert(contract.id.as_str()) {
        return validation_error(INVALID_CONTRACT_ID);
    }

    if contract.revisions.is_empty() {
        return validation_error(EMPTY_CONTRACT_REVISIONS);
    }

    let mut revision_versions = HashSet::new();
    for revision in &contract.revisions {
        validate_runtime_contract_revision(revision, &mut revision_versions)?;
    }

    Ok(())
}

fn validate_runtime_contract_revision<'contract>(
    revision: &'contract RuntimeContractRevision,
    revision_versions: &mut HashSet<&'contract str>,
) -> Result<(), RuntimeCatalogError> {
    if revision.version.trim().is_empty()
        || !revision_versions.insert(revision.version.as_str())
        || revision.image_ref.trim().is_empty()
    {
        return validation_error(INVALID_CONTRACT_REVISION);
    }

    Ok(())
}

fn validation_error<T>(message: &'static str) -> Result<T, RuntimeCatalogError> {
    Err(RuntimeCatalogError::ValidationFailed {
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn validate_runtime_catalog_accepts_valid_catalog() {
        assert_eq!(
            validate_runtime_catalog(&runtime_catalog(
                "runpod-endpoint-comfyui-hidream-o1-dev",
                "1.0.15"
            )),
            Ok(())
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_empty_catalog() {
        assert_eq!(
            validate_runtime_catalog(&RuntimeCatalog { contracts: vec![] }),
            Err(RuntimeCatalogError::ValidationFailed {
                message: "catalog is empty".to_string()
            })
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
            Err(RuntimeCatalogError::ValidationFailed {
                message: "contract ID is empty or duplicate".to_string()
            })
        );
    }

    #[test]
    fn validate_runtime_catalog_rejects_empty_revision_image_ref() {
        let mut catalog = runtime_catalog("runpod-endpoint-comfyui-hidream-o1-dev", "1.0.15");
        catalog.contracts[0].revisions[0].image_ref = " ".to_string();

        assert_eq!(
            validate_runtime_catalog(&catalog),
            Err(RuntimeCatalogError::ValidationFailed {
                message: "revision version is empty, duplicate, or image reference is empty"
                    .to_string()
            })
        );
    }
}
