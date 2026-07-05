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

    #[test]
    fn list_returns_runtime_contracts() {
        let repository = BundledRuntimeContractRepository::new();
        let contracts = repository.list().expect("runtime contracts should parse");

        assert_eq!(contracts.len(), 2);
    }
}
