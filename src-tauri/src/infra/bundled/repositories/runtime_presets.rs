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

    #[test]
    fn list_returns_runtime_presets() {
        let repository = BundledRuntimePresetRepository::new();
        let presets = repository.list().expect("runtime presets should parse");

        assert_eq!(presets.len(), 1);
    }
}
