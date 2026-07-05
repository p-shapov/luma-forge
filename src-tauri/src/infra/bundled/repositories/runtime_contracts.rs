use super::super::{catalog::parse_asset, errors::BundledCatalogError, BundledCatalog};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimeContractRepository {
    catalog: BundledCatalog,
}

impl BundledRuntimeContractRepository {
    pub fn new() -> Self {
        Self {
            catalog: BundledCatalog::new(),
        }
    }

    #[cfg(test)]
    pub fn from_catalog(catalog: BundledCatalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Result<Vec<serde_json::Value>, BundledCatalogError> {
        self.catalog
            .assets()
            .iter()
            .filter(|(path, _)| path.starts_with("runtime_contracts/"))
            .map(|(path, text)| parse_asset(path, text))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE_ASSETS: &[(&str, &str)] = &[
        ("runtime_contracts/contract-a/1.0.0.json", "{}"),
        ("runtime_contracts/contract-b/2.0.0.json", "{}"),
    ];

    #[test]
    fn list_returns_runtime_contracts() {
        let repository = BundledRuntimeContractRepository::from_catalog(
            BundledCatalog::from_assets(FIXTURE_ASSETS),
        );
        let contracts = repository.list().expect("runtime contracts should parse");

        assert_eq!(contracts.len(), 2);
    }
}
