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

fn identity_from_revision_path(
    path: &str,
    prefix: &str,
) -> Result<(String, String), BundledCatalogError> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [actual_prefix, id, file] if *actual_prefix == prefix => {
            let Some(revision) = file.strip_suffix(".json") else {
                return Err(BundledCatalogError::corrupt_asset(
                    path,
                    "revision file is invalid",
                ));
            };
            Ok(((*id).to_string(), revision.to_string()))
        }
        _ => Err(BundledCatalogError::corrupt_asset(
            path,
            "bundled path is invalid",
        )),
    }
}

fn parse_runtime_preset(
    path: &str,
    text: &str,
) -> Result<BundledRuntimePreset, BundledCatalogError> {
    let (id, revision) = identity_from_revision_path(path, "runtime_presets")?;
    let preset: generated::RuntimePreset = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    Ok(BundledRuntimePreset {
        id,
        revision,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_preset_uses_identity_from_path() {
        let preset = parse_runtime_preset(
            "runtime_presets/example/1.2.3.json",
            r#"{
              "$schema":"luma-forge://schemas/bundled/runtime_preset.schema.json",
              "runtime":{
                "python_version":"3.12",
                "comfyui_revision":"abc123",
                "pytorch":{
                  "index_url":"https://download.pytorch.org/whl/cu126",
                  "packages":["torch==2.9.1"]
                }
              }
            }"#,
        )
        .expect("preset should parse");

        assert_eq!(preset.id, "example");
        assert_eq!(preset.revision, "1.2.3");
    }

    #[test]
    fn get_returns_none_for_missing_runtime_preset() {
        let repository = BundledRuntimePresetRepository::new();

        assert_eq!(
            repository
                .get("missing-runtime-preset", "9.9.9")
                .expect("lookup should succeed"),
            None
        );
    }
}
