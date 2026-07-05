use crate::infra::bundled::{
    errors::BundledCatalogError, generated, models::BundledRuntimeContract,
};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimeContractRepository;

impl BundledRuntimeContractRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledRuntimeContract>, BundledCatalogError> {
        generated::BUNDLED_ASSETS
            .iter()
            .filter(|(path, _)| path.starts_with("runtime_contracts/"))
            .map(|(path, text)| parse_runtime_contract(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimeContract>, BundledCatalogError> {
        let path = format!("runtime_contracts/{id}/{revision}.json");
        generated::BUNDLED_ASSETS
            .iter()
            .find_map(|(asset_path, text)| (*asset_path == path).then_some(*text))
            .map(|text| parse_runtime_contract(&path, text))
            .transpose()
    }
}

fn identity_from_revision_path(
    path: &str,
    prefix: &str,
) -> Result<(String, String), BundledCatalogError> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [actual_prefix, id, file] if *actual_prefix == prefix => {
            let Some(revision) = file.strip_suffix(".json") else {
                return Err(BundledCatalogError::corrupt_asset(
                    path,
                    "revision file is invalid",
                ));
            };
            Ok(((*id).to_string(), revision.to_string()))
        }
        _ => Err(BundledCatalogError::corrupt_asset(
            path,
            "bundled path is invalid",
        )),
    }
}

fn parse_runtime_contract(
    path: &str,
    text: &str,
) -> Result<BundledRuntimeContract, BundledCatalogError> {
    let contract: generated::RuntimeContract = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    let (id, revision) = identity_from_revision_path(path, "runtime_contracts")?;
    Ok(BundledRuntimeContract {
        id,
        revision,
        image_ref: contract.image_ref.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_contract_uses_identity_from_path() {
        let contract = parse_runtime_contract(
            "runtime_contracts/example/1.2.3.json",
            r#"{
              "$schema":"luma-forge://schemas/bundled/runtime_contract.schema.json",
              "image_ref":"ghcr.io/example/image@sha256:abc123"
            }"#,
        )
        .expect("contract should parse");

        assert_eq!(contract.id, "example");
        assert_eq!(contract.revision, "1.2.3");
    }

    #[test]
    fn get_returns_none_for_missing_runtime_contract() {
        let repository = BundledRuntimeContractRepository::new();

        assert_eq!(
            repository
                .get("missing-runtime-contract", "9.9.9")
                .expect("lookup should succeed"),
            None
        );
    }
}
