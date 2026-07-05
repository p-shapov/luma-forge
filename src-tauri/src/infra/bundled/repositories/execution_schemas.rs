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

    #[cfg(test)]
    pub fn from_catalog(catalog: BundledCatalog) -> Self {
        Self { catalog }
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

    static FIXTURE_ASSETS: &[(&str, &str)] = &[("execution_schemas/schema-a/1.0.0.json", "{}")];

    #[test]
    fn list_returns_execution_schemas() {
        let repository = BundledExecutionSchemaRepository::from_catalog(
            BundledCatalog::from_assets(FIXTURE_ASSETS),
        );
        let schemas = repository.list().expect("execution schemas should parse");

        assert_eq!(schemas.len(), 1);
    }
}
