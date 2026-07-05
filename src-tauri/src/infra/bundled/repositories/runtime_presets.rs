use super::super::{
    catalog::parse_asset, errors::BundledCatalogError, generated::RuntimePreset, BundledCatalog,
};

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

    pub fn list(&self) -> Result<Vec<RuntimePreset>, BundledCatalogError> {
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

    fn assert_runtime_presets(_: &[crate::infra::bundled::generated::RuntimePreset]) {}

    static FIXTURE_ASSETS: &[(&str, &str)] = &[(
        "runtime_presets/base/1.0.0.json",
        r#"{"$schema":"luma-forge://schemas/bundled/runtime_preset.schema.json","id":"base","revision":"1.0.0","runtime":{"python_version":"3.11","comfyui_revision":"abc123","pytorch":{"index_url":"https://example.com","packages":["torch"]}}}"#,
    )];

    #[test]
    fn list_returns_runtime_presets() {
        let repository = BundledRuntimePresetRepository::from_catalog(BundledCatalog::from_assets(
            FIXTURE_ASSETS,
        ));
        let presets = repository.list().expect("runtime presets should parse");
        assert_runtime_presets(&presets);

        assert_eq!(presets.len(), 1);
    }
}
