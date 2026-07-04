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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    pub id: String,
    pub json: serde_json::Value,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

        let text = std::fs::read_to_string(&path)
            .map_err(|error| invalid(&relative, format!("bundled read failed: {error}")))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| invalid(&relative, format!("bundled JSON parse failed: {error}")))?;
        let schema_id = json
            .get("$schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid(&relative, "bundled JSON missing $schema"))?;
        let schema = schemas
            .iter()
            .find(|schema| schema.id == schema_id)
            .ok_or_else(|| invalid(&relative, format!("unknown bundled schema {schema_id}")))?;
        let validator = jsonschema::validator_for(&schema.json)
            .map_err(|error| invalid(&relative, format!("schema validator failed: {error}")))?;
        if let Err(error) = validator.validate(&json) {
            return Err(invalid(
                &relative,
                format!("bundled schema validation failed: {error}"),
            ));
        }

        assets.push(BundledAsset {
            path: relative,
            schema_id: schema_id.to_string(),
            json,
        });
    }

    Ok(assets)
}

fn sorted_json_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files);
    files.sort();
    files
}

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

fn invalid(path: &str, message: impl Into<String>) -> BundledValidationError {
    BundledValidationError::Invalid {
        path: path.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn validate_bundled_catalog_accepts_valid_fixture() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{
              "$schema": "https://example.com/schemas/workflow-metadata.json",
              "name": "Example Flow"
            }"#,
        );

        let assets = validate_bundled_catalog(fixture.path(), &test_schemas()).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].path, "workflows/example-flow/1.0.0/metadata.json");
        assert_eq!(
            assets[0].schema_id,
            "https://example.com/schemas/workflow-metadata.json"
        );
    }

    #[test]
    fn validate_bundled_catalog_returns_err_for_invalid_json() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{"$schema":"https://example.com/schemas/workflow-metadata.json""#,
        );

        let error = validate_bundled_catalog(fixture.path(), &test_schemas()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, "workflows/example-flow/1.0.0/metadata.json");
        assert!(message.starts_with("bundled JSON parse failed:"));
    }

    fn test_schemas() -> Vec<SchemaDocument> {
        vec![SchemaDocument {
            id: "https://example.com/schemas/workflow-metadata.json".to_string(),
            json: serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$id": "https://example.com/schemas/workflow-metadata.json",
                "type": "object",
                "required": ["$schema", "name"],
                "properties": {
                    "$schema": { "const": "https://example.com/schemas/workflow-metadata.json" },
                    "name": { "type": "string" }
                },
                "additionalProperties": false
            }),
        }]
    }

    struct TestFixture {
        root: PathBuf,
    }

    impl TestFixture {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);

            let unique = format!(
                "luma-forge-validation-{}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before epoch")
                    .as_nanos(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir_all(&root).expect("failed to create test fixture root");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_json(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            let parent = path.parent().expect("fixture file should have a parent");
            fs::create_dir_all(parent).expect("failed to create fixture parent");
            fs::write(path, contents).expect("failed to write fixture JSON");
        }
    }

    impl Drop for TestFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
