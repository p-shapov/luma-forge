use super::super::{
    catalog::parse_asset, errors::BundledCatalogError, generated::ExecutionSchema, BundledCatalog,
};

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

    pub fn list(&self) -> Result<Vec<ExecutionSchema>, BundledCatalogError> {
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

    fn assert_execution_schemas(_: &[crate::infra::bundled::generated::ExecutionSchema]) {}

    static FIXTURE_ASSETS: &[(&str, &str)] = &[(
        "execution_schemas/schema-a/1.0.0.json",
        r#"{"$schema":"luma-forge://schemas/bundled/execution_schema.schema.json","id":"schema-a","revision":"1.0.0","inputs":[{"id":"prompt","type":"string","required":true}],"outputs":{"type":"image"}}"#,
    )];

    #[test]
    fn list_returns_execution_schemas() {
        let repository = BundledExecutionSchemaRepository::from_catalog(
            BundledCatalog::from_assets(FIXTURE_ASSETS),
        );
        let schemas = repository.list().expect("execution schemas should parse");
        assert_execution_schemas(&schemas);

        assert_eq!(schemas.len(), 1);
    }
}
