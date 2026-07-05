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

fn parse_runtime_contract(
    path: &str,
    text: &str,
) -> Result<BundledRuntimeContract, BundledCatalogError> {
    let contract: generated::RuntimeContract = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    Ok(BundledRuntimeContract {
        id: contract.id.into(),
        revision: contract.revision.into(),
        image_ref: contract.image_ref.into(),
    })
}
