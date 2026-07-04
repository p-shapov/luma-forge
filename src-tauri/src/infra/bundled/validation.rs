#[cfg(not(test))]
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledAsset {
    pub path: String,
    pub schema_id: String,
    pub json: serde_json::Value,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BundledValidationError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
}

#[cfg(not(test))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    pub id: String,
    pub json: serde_json::Value,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_safe_relative_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_secret_like(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("secret")
        || value.contains("token")
        || value.contains("password")
        || value.contains("api_key")
        || value.contains("apikey")
        || value.contains("credential")
}

pub fn approved_bundled_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["workflows", workflow_id, revision, file] => {
            safe_id(workflow_id)
                && safe_revision(revision)
                && matches!(
                    *file,
                    "metadata.json"
                        | "model_assets.json"
                        | "contract_requirements.json"
                        | "execution_contract.json"
                        | "workflow.json"
                )
        }
        ["runtime_presets", runtime_preset_id, file] => {
            safe_id(runtime_preset_id) && file.ends_with(".json")
        }
        ["runtime_contracts", contract_id, file] => safe_id(contract_id) && file.ends_with(".json"),
        ["execution_schemas", schema_id, file] => safe_id(schema_id) && file.ends_with(".json"),
        _ => false,
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn safe_revision(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

pub fn validate_cross_file_assets(assets: &[BundledAsset]) -> Result<(), BundledValidationError> {
    for asset in assets {
        if !approved_bundled_path(&asset.path) {
            return Err(BundledValidationError::Invalid {
                path: asset.path.clone(),
                message: "unexpected bundled JSON path".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(not(test))]
pub fn validate_bundled_catalog(
    root: &Path,
    schemas: &[SchemaDocument],
) -> Result<Vec<BundledAsset>, BundledValidationError> {
    let mut assets = Vec::new();

    for path in sorted_json_files(root) {
        let relative = path
            .strip_prefix(root)
            .expect("bundled path should be under bundled root")
            .to_string_lossy()
            .replace('\\', "/");
        if !approved_bundled_path(&relative) {
            return Err(BundledValidationError::Invalid {
                path: relative,
                message: "unexpected bundled JSON path".to_string(),
            });
        }

        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{}: bundled read failed: {error}",
                path.strip_prefix(root)
                    .expect("bundled path should be under bundled root")
                    .display()
            )
        });
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "{}: bundled JSON parse failed: {error}",
                path.strip_prefix(root)
                    .expect("bundled path should be under bundled root")
                    .display()
            )
        });
        let schema_id = json
            .get("$schema")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{relative}: bundled JSON missing $schema"));
        let schema = schemas
            .iter()
            .find(|schema| schema.id == schema_id)
            .unwrap_or_else(|| panic!("{relative}: unknown bundled schema {schema_id}"));
        let validator = jsonschema::validator_for(&schema.json)
            .unwrap_or_else(|error| panic!("{schema_id}: schema validator failed: {error}"));
        if let Err(error) = validator.validate(&json) {
            panic!("{relative}: bundled schema validation failed: {error}");
        }

        assets.push(BundledAsset {
            path: relative,
            schema_id: schema_id.to_string(),
            json,
        });
    }

    Ok(assets)
}

#[cfg(not(test))]
fn sorted_json_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files);
    files.sort();
    files
}

#[cfg(not(test))]
fn collect_json_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            files.push(path.to_path_buf());
        }
        return;
    }

    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };

    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        collect_json_files(&entry.path(), files);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_path_rejects_absolute_parent_and_backslash_paths() {
        assert!(is_safe_relative_path(
            "models/checkpoints/model.safetensors"
        ));
        assert!(!is_safe_relative_path("../outside.safetensors"));
        assert!(!is_safe_relative_path("/absolute.safetensors"));
        assert!(!is_safe_relative_path(
            "models\\checkpoints\\model.safetensors"
        ));
    }

    #[test]
    fn secret_like_rejects_credential_names() {
        assert!(is_secret_like("api_key"));
        assert!(is_secret_like("worker_token"));
        assert!(!is_secret_like("prompt"));
    }

    #[test]
    fn approved_paths_accept_only_new_bundled_tree_shapes() {
        assert!(approved_bundled_path(
            "workflows/example-flow/1.0.0/metadata.json"
        ));
        assert!(approved_bundled_path(
            "runtime_presets/comfyui-py312-cu126-torch291/1.0.0.json"
        ));
        assert!(!approved_bundled_path("workflow-catalog.json"));
        assert!(!approved_bundled_path("workflows/example-flow.json"));
    }
}
