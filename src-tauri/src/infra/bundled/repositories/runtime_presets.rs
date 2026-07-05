use super::super::{catalog::parse_asset, errors::BundledCatalogError, BundledCatalog};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimePresetRepository {
    catalog: BundledCatalog,
}

impl BundledRuntimePresetRepository {
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
            .filter(|(path, _)| path.starts_with("runtime_presets/"))
            .map(|(path, text)| parse_asset(path, text))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE_ASSETS: &[(&str, &str)] = &[("runtime_presets/base/1.0.0.json", "{}")];

    #[test]
    fn list_returns_runtime_presets() {
        let repository = BundledRuntimePresetRepository::from_catalog(BundledCatalog::from_assets(
            FIXTURE_ASSETS,
        ));
        let presets = repository.list().expect("runtime presets should parse");

        assert_eq!(presets.len(), 1);
    }
}
