use super::super::{catalog::parse_asset, errors::BundledCatalogError, BundledCatalog};

#[derive(Debug, Clone, Default)]
pub struct BundledExecutionSchemaRepository {
    catalog: BundledCatalog,
}

impl BundledExecutionSchemaRepository {
    pub fn new() -> Self {
        Self {
            catalog: BundledCatalog::new(),
        }
    }

    pub fn list(&self) -> Result<Vec<serde_json::Value>, BundledCatalogError> {
        self.catalog
            .assets()
            .iter()
            .filter(|(path, _)| path.starts_with("execution_schemas/"))
            .map(|(path, text)| parse_asset(path, text))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_execution_schemas() {
        let repository = BundledExecutionSchemaRepository::new();
        let schemas = repository.list().expect("execution schemas should parse");

        assert_eq!(schemas.len(), 1);
    }
}
