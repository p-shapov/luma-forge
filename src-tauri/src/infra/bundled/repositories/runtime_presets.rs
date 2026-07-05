use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{BundledRuntimePreset, BundledRuntimePresetPytorch, BundledRuntimePresetRuntime},
};

#[derive(Debug, Clone, Default)]
pub struct BundledRuntimePresetRepository;

impl BundledRuntimePresetRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledRuntimePreset>, BundledCatalogError> {
        generated::BUNDLED_ASSETS
            .iter()
            .filter(|(path, _)| path.starts_with("runtime_presets/"))
            .map(|(path, text)| parse_runtime_preset(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimePreset>, BundledCatalogError> {
        let path = format!("runtime_presets/{id}/{revision}.json");
        generated::BUNDLED_ASSETS
            .iter()
            .find_map(|(asset_path, text)| (*asset_path == path).then_some(*text))
            .map(|text| parse_runtime_preset(&path, text))
            .transpose()
    }
}

fn parse_runtime_preset(
    path: &str,
    text: &str,
) -> Result<BundledRuntimePreset, BundledCatalogError> {
    let preset: generated::RuntimePreset = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    Ok(BundledRuntimePreset {
        id: preset.id.into(),
        revision: preset.revision.into(),
        runtime: BundledRuntimePresetRuntime {
            python_version: preset.runtime.python_version.into(),
            comfyui_revision: preset.runtime.comfyui_revision.into(),
            pytorch: BundledRuntimePresetPytorch {
                index_url: preset.runtime.pytorch.index_url.into(),
                packages: preset
                    .runtime
                    .pytorch
                    .packages
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            },
        },
    })
}
