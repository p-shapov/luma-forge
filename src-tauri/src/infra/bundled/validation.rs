use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::validation_errors::BundledValidationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledAsset {
    pub path: String,
    pub schema_id: String,
    pub json: serde_json::Value,
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
            safe_id(runtime_preset_id) && safe_revision_file(file)
        }
        ["runtime_contracts", contract_id, file] => {
            safe_id(contract_id) && safe_revision_file(file)
        }
        ["execution_schemas", schema_id, file] => safe_id(schema_id) && safe_revision_file(file),
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

fn safe_revision_file(file: &str) -> bool {
    file.strip_suffix(".json").is_some_and(safe_revision)
}

pub fn validate_cross_file_assets(assets: &[BundledAsset]) -> Result<(), BundledValidationError> {
    let mut workflow_files: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut runtime_presets = BTreeSet::new();
    let mut runtime_contracts = BTreeSet::new();
    let mut execution_schemas = BTreeSet::new();
    let mut execution_schema_inputs = BTreeMap::new();

    for asset in assets {
        if !approved_bundled_path(&asset.path) {
            return Err(invalid(&asset.path, "unexpected bundled JSON path"));
        }
        let parts: Vec<&str> = asset.path.split('/').collect();
        match parts.as_slice() {
            ["workflows", workflow_id, revision, file] => {
                let files = workflow_files
                    .entry((workflow_id.to_string(), revision.to_string()))
                    .or_default();
                if !files.insert(file.to_string()) {
                    return Err(invalid(&asset.path, "duplicate workflow file identity"));
                }
            }
            ["runtime_presets", id, file] => {
                let key = (id.to_string(), file.trim_end_matches(".json").to_string());
                if !runtime_presets.insert(key) {
                    return Err(invalid(&asset.path, "duplicate runtime preset identity"));
                }
            }
            ["runtime_contracts", id, file] => {
                let key = (id.to_string(), file.trim_end_matches(".json").to_string());
                if !runtime_contracts.insert(key) {
                    return Err(invalid(&asset.path, "duplicate runtime contract identity"));
                }
            }
            ["execution_schemas", id, file] => {
                let key = (id.to_string(), file.trim_end_matches(".json").to_string());
                if !execution_schemas.insert(key.clone()) {
                    return Err(invalid(&asset.path, "duplicate execution schema identity"));
                }
                execution_schema_inputs.insert(key, execution_input_ids(asset));
            }
            _ => return Err(invalid(&asset.path, "unexpected bundled JSON path")),
        }
        reject_path_identity(asset, &parts)?;
        reject_unsafe_model_paths(asset)?;
    }

    let required = BTreeSet::from([
        "metadata.json".to_string(),
        "model_assets.json".to_string(),
        "contract_requirements.json".to_string(),
        "execution_contract.json".to_string(),
        "workflow.json".to_string(),
    ]);
    for ((workflow_id, revision), files) in workflow_files {
        if files != required {
            return Err(invalid(
                &format!("workflows/{workflow_id}/{revision}"),
                "workflow revision directory does not contain the required five files",
            ));
        }
    }

    for asset in assets {
        reject_workflow_references(
            asset,
            &runtime_presets,
            &runtime_contracts,
            &execution_schemas,
            &execution_schema_inputs,
        )?;
    }
    Ok(())
}

fn reject_path_identity(
    asset: &BundledAsset,
    parts: &[&str],
) -> Result<(), BundledValidationError> {
    match parts {
        ["workflows", workflow_id, revision, "metadata.json"] => {
            expect_identity(asset, workflow_id, revision, "id")
        }
        ["workflows", workflow_id, revision, _] => {
            expect_identity(asset, workflow_id, revision, "workflow_id")
        }
        ["runtime_presets", id, file]
        | ["runtime_contracts", id, file]
        | ["execution_schemas", id, file] => {
            expect_identity(asset, id, file.trim_end_matches(".json"), "id")
        }
        _ => Err(invalid(&asset.path, "unexpected bundled JSON path")),
    }
}

fn expect_identity(
    asset: &BundledAsset,
    expected_id: &str,
    expected_revision: &str,
    id_field: &str,
) -> Result<(), BundledValidationError> {
    let actual_id = asset
        .json
        .get(id_field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let actual_revision = asset
        .json
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if actual_id != expected_id || actual_revision != expected_revision {
        return Err(invalid(
            &asset.path,
            "bundled asset identity does not match its path",
        ));
    }
    Ok(())
}

fn reject_unsafe_model_paths(asset: &BundledAsset) -> Result<(), BundledValidationError> {
    if asset.schema_id != "luma-forge://schemas/bundled/workflow_model_assets.schema.json" {
        return Ok(());
    }
    let Some(model_assets) = asset
        .json
        .get("model_assets")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for model_asset in model_assets {
        let install_path = model_asset
            .get("install_comfyui_relative_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let source_path = model_asset
            .get("download_source")
            .and_then(|source| source.get("file_path"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !is_safe_relative_path(install_path) || !is_safe_relative_path(source_path) {
            return Err(invalid(&asset.path, "model asset path is unsafe"));
        }
    }
    Ok(())
}

fn execution_input_ids(asset: &BundledAsset) -> BTreeMap<String, bool> {
    asset
        .json
        .get("inputs")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|input| {
            let id = input.get("id").and_then(serde_json::Value::as_str)?;
            let required = input
                .get("required")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some((id.to_string(), required))
        })
        .collect()
}

fn reject_workflow_references(
    asset: &BundledAsset,
    runtime_presets: &BTreeSet<(String, String)>,
    runtime_contracts: &BTreeSet<(String, String)>,
    execution_schemas: &BTreeSet<(String, String)>,
    execution_schema_inputs: &BTreeMap<(String, String), BTreeMap<String, bool>>,
) -> Result<(), BundledValidationError> {
    match asset.schema_id.as_str() {
        "luma-forge://schemas/bundled/workflow_metadata.schema.json" => {
            let Some(reference) = asset.json.get("runtime_preset") else {
                return Ok(());
            };
            let key = reference_key(reference);
            if !runtime_presets.contains(&key) {
                return Err(invalid(
                    &asset.path,
                    "workflow metadata references an unknown runtime preset",
                ));
            }
        }
        "luma-forge://schemas/bundled/workflow_contract_requirements.schema.json" => {
            let Some(requirements) = asset
                .json
                .get("contract_requirements")
                .and_then(serde_json::Value::as_array)
            else {
                return Ok(());
            };
            for requirement in requirements {
                for field in ["endpoint_contract", "provisioner_contract"] {
                    let Some(reference) = requirement.get(field) else {
                        continue;
                    };
                    let key = reference_key(reference);
                    if !runtime_contracts.contains(&key) {
                        return Err(invalid(
                            &asset.path,
                            "workflow contract requirements reference an unknown runtime contract",
                        ));
                    }
                }
            }
        }
        "luma-forge://schemas/bundled/workflow_execution_contract.schema.json" => {
            let Some(schema_ref) = asset.json.get("schema_ref") else {
                return Ok(());
            };
            let key = reference_key(schema_ref);
            if !execution_schemas.contains(&key) {
                return Err(invalid(
                    &asset.path,
                    "workflow execution contract references an unknown execution schema",
                ));
            }
            let Some(known_inputs) = execution_schema_inputs.get(&key) else {
                return Err(invalid(
                    &asset.path,
                    "workflow execution contract references an unknown execution schema",
                ));
            };
            let Some(bindings) = asset
                .json
                .get("input_bindings")
                .and_then(serde_json::Value::as_array)
            else {
                return Ok(());
            };
            let mut bound_inputs = BTreeSet::new();
            for binding in bindings {
                let Some(input_id) = template_input_id(
                    &asset.path,
                    binding.get("value").unwrap_or(&serde_json::Value::Null),
                )?
                else {
                    continue;
                };
                if !known_inputs.contains_key(input_id.as_str()) {
                    return Err(invalid(
                        &asset.path,
                        "workflow execution contract references an unknown execution input",
                    ));
                }
                bound_inputs.insert(input_id);
            }
            for (input_id, required) in known_inputs {
                if *required && !bound_inputs.contains(input_id) {
                    return Err(invalid(
                        &asset.path,
                        "workflow execution contract is missing a required input binding",
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn reference_key(value: &serde_json::Value) -> (String, String) {
    let id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let revision = value
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    (id, revision)
}

fn template_input_id(
    path: &str,
    value: &serde_json::Value,
) -> Result<Option<String>, BundledValidationError> {
    let serde_json::Value::String(text) = value else {
        return Ok(None);
    };
    let starts = text.starts_with("{{");
    let ends = text.ends_with("}}");
    if !starts && !ends {
        return Ok(None);
    }
    if !(starts && ends) || text.len() <= 4 {
        return Err(invalid(
            path,
            "workflow execution contract input binding template is malformed",
        ));
    }
    let inner = &text[2..text.len() - 2];
    if inner.trim() != inner || inner.is_empty() || inner.contains('{') || inner.contains('}') {
        return Err(invalid(
            path,
            "workflow execution contract input binding template is malformed",
        ));
    }
    Ok(Some(inner.to_string()))
}

pub fn validate_bundled_catalog(
    root: &Path,
    schemas: &[SchemaDocument],
) -> Result<Vec<BundledAsset>, BundledValidationError> {
    let mut assets = Vec::new();

    for path in sorted_json_files(root)? {
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

fn sorted_json_files(root: &Path) -> Result<Vec<PathBuf>, BundledValidationError> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), BundledValidationError> {
    if path.is_file() {
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    let path_string = path.display().to_string();
    let entries = std::fs::read_dir(path).map_err(|error| {
        invalid(
            &path_string,
            format!("bundled directory traversal failed: {error}"),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            invalid(
                &path_string,
                format!("bundled directory entry failed: {error}"),
            )
        })?;
        collect_json_files(&entry.path(), files)?;
    }
    Ok(())
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
    fn approved_paths_accept_only_new_bundled_tree_shapes() {
        assert!(approved_bundled_path(
            "workflows/example-flow/1.0.0/metadata.json"
        ));
        assert!(approved_bundled_path(
            "runtime_presets/comfyui-py312-cu126-torch291/1.0.0.json"
        ));
        assert!(approved_bundled_path(
            "runtime_contracts/contract-a/2.3.4.json"
        ));
        assert!(approved_bundled_path(
            "execution_schemas/schema-a/9.8.7.json"
        ));
        assert!(!approved_bundled_path("runtime_presets/foo/catalog.json"));
        assert!(!approved_bundled_path("runtime_contracts/foo/catalog.json"));
        assert!(!approved_bundled_path("execution_schemas/foo/catalog.json"));
        assert!(!approved_bundled_path("workflow-catalog.json"));
        assert!(!approved_bundled_path("workflows/example-flow.json"));
    }

    #[test]
    fn validate_bundled_catalog_accepts_valid_fixture() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{
              "$schema": "luma-forge://schemas/bundled/workflow_metadata.schema.json",
              "name": "Example Flow"
            }"#,
        );

        let assets = validate_bundled_catalog(fixture.path(), &test_schemas()).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].path, "workflows/example-flow/1.0.0/metadata.json");
        assert_eq!(
            assets[0].schema_id,
            "luma-forge://schemas/bundled/workflow_metadata.schema.json"
        );
    }

    #[test]
    fn validate_bundled_catalog_returns_err_for_invalid_json() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{"$schema":"luma-forge://schemas/bundled/workflow_metadata.schema.json""#,
        );

        let error = validate_bundled_catalog(fixture.path(), &test_schemas()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, "workflows/example-flow/1.0.0/metadata.json");
        assert!(message.starts_with("bundled JSON parse failed:"));
    }

    #[test]
    fn validate_bundled_catalog_returns_err_for_missing_root() {
        let fixture = TestFixture::new();
        let root = fixture.path().to_path_buf();
        drop(fixture);

        let error = validate_bundled_catalog(&root, &test_schemas()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, root.display().to_string());
        assert!(message.starts_with("bundled directory traversal failed:"));
    }

    #[test]
    fn validate_bundled_catalog_uses_json_schema_as_source_of_truth() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{
              "$schema": "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
              "model_assets": []
            }"#,
        );

        let assets = validate_bundled_catalog(fixture.path(), &test_schemas()).unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(
            assets[0].schema_id,
            "luma-forge://schemas/bundled/workflow_model_assets.schema.json"
        );
    }

    fn test_schemas() -> Vec<SchemaDocument> {
        vec![
            SchemaDocument {
                id: "luma-forge://schemas/bundled/workflow_metadata.schema.json".to_string(),
                json: serde_json::json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "$id": "luma-forge://schemas/bundled/workflow_metadata.schema.json",
                    "type": "object",
                    "required": ["$schema", "name"],
                    "properties": {
                        "$schema": {
                            "const": "luma-forge://schemas/bundled/workflow_metadata.schema.json"
                        },
                        "name": { "type": "string" }
                    },
                    "additionalProperties": false
                }),
            },
            SchemaDocument {
                id: "luma-forge://schemas/bundled/workflow_model_assets.schema.json".to_string(),
                json: serde_json::json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "$id": "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
                    "type": "object",
                    "required": ["$schema", "model_assets"],
                    "properties": {
                        "$schema": {
                            "const": "luma-forge://schemas/bundled/workflow_model_assets.schema.json"
                        },
                        "model_assets": { "type": "array" }
                    },
                    "additionalProperties": false
                }),
            },
        ]
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

#[cfg(test)]
mod cross_file_tests {
    use super::*;
    use serde_json::json;

    fn asset(path: &str, schema_id: &str, json: serde_json::Value) -> BundledAsset {
        BundledAsset {
            path: path.to_string(),
            schema_id: schema_id.to_string(),
            json,
        }
    }

    fn valid_assets() -> Vec<BundledAsset> {
        vec![
            asset(
                "runtime_presets/base/1.0.0.json",
                "luma-forge://schemas/bundled/runtime_preset.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/runtime_preset.schema.json",
                    "id": "base",
                    "revision": "1.0.0",
                    "runtime": {
                        "python_version": "3.12",
                        "comfyui_revision": "abc123",
                        "pytorch": {
                            "index_url": "https://example.com",
                            "packages": ["torch==1.0.0"]
                        }
                    }
                }),
            ),
            asset(
                "runtime_contracts/endpoint/1.0.0.json",
                "luma-forge://schemas/bundled/runtime_contract.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/runtime_contract.schema.json",
                    "id": "endpoint",
                    "revision": "1.0.0",
                    "image_ref": "ghcr.io/example/endpoint:latest"
                }),
            ),
            asset(
                "runtime_contracts/provisioner/1.0.0.json",
                "luma-forge://schemas/bundled/runtime_contract.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/runtime_contract.schema.json",
                    "id": "provisioner",
                    "revision": "1.0.0",
                    "image_ref": "ghcr.io/example/provisioner:latest"
                }),
            ),
            asset(
                "execution_schemas/text-to-image/1.0.0.json",
                "luma-forge://schemas/bundled/execution_schema.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/execution_schema.schema.json",
                    "id": "text-to-image",
                    "revision": "1.0.0",
                    "inputs": [{ "id": "prompt", "type": "string", "required": true }],
                    "outputs": { "type": "image_set" }
                }),
            ),
            asset(
                "workflows/example/1.0.0/metadata.json",
                "luma-forge://schemas/bundled/workflow_metadata.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_metadata.schema.json",
                    "id": "example",
                    "revision": "1.0.0",
                    "name": "Example",
                    "runtime_preset": { "id": "base", "revision": "1.0.0" },
                    "requires_hugging_face_api_key": false,
                    "required_volume_size_gb": 1
                }),
            ),
            asset(
                "workflows/example/1.0.0/model_assets.json",
                "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
                    "workflow_id": "example",
                    "revision": "1.0.0",
                    "model_assets": [{
                        "id": "asset",
                        "name": "Asset",
                        "download_source": {
                            "source_type": "huggingface",
                            "repository_id": "owner/repo",
                            "file_path": "models/model.safetensors",
                            "revision": "main"
                        },
                        "install_comfyui_relative_path": "models/model.safetensors"
                    }]
                }),
            ),
            asset(
                "workflows/example/1.0.0/contract_requirements.json",
                "luma-forge://schemas/bundled/workflow_contract_requirements.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_contract_requirements.schema.json",
                    "workflow_id": "example",
                    "revision": "1.0.0",
                    "contract_requirements": [{
                        "runtime_type": "runpod",
                        "endpoint_contract": { "id": "endpoint", "revision": "1.0.0" },
                        "provisioner_contract": { "id": "provisioner", "revision": "1.0.0" }
                    }]
                }),
            ),
            asset(
                "workflows/example/1.0.0/execution_contract.json",
                "luma-forge://schemas/bundled/workflow_execution_contract.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_execution_contract.schema.json",
                    "workflow_id": "example",
                    "revision": "1.0.0",
                    "schema_ref": { "id": "text-to-image", "revision": "1.0.0" },
                    "input_bindings": [{
                        "value": "{{prompt}}",
                        "node_id": "1",
                        "path": ["inputs", "prompt"]
                    }]
                }),
            ),
            asset(
                "workflows/example/1.0.0/workflow.json",
                "luma-forge://schemas/bundled/workflow_graph.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_graph.schema.json",
                    "workflow_id": "example",
                    "revision": "1.0.0",
                    "graph": {}
                }),
            ),
        ]
    }

    #[test]
    fn validation_accepts_valid_cross_file_assets() {
        assert_eq!(validate_cross_file_assets(&valid_assets()), Ok(()));
    }

    #[test]
    fn validation_rejects_unsafe_model_asset_paths() {
        let assets = vec![asset(
            "workflows/example/1.0.0/model_assets.json",
            "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
            json!({
                "$schema": "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
                "workflow_id": "example",
                "revision": "1.0.0",
                "model_assets": [{
                    "id": "asset",
                    "name": "Asset",
                    "download_source": {
                        "source_type": "huggingface",
                        "repository_id": "owner/repo",
                        "file_path": "../model.safetensors",
                        "revision": "main"
                    },
                    "install_comfyui_relative_path": "models/model.safetensors"
                }]
            }),
        )];

        assert_eq!(
            validate_cross_file_assets(&assets),
            Err(BundledValidationError::Invalid {
                path: "workflows/example/1.0.0/model_assets.json".to_string(),
                message: "model asset path is unsafe".to_string(),
            })
        );
    }

    #[test]
    fn validation_rejects_workflow_path_identity_mismatches() {
        let mut assets = valid_assets();
        assets[4].json["id"] = json!("other");

        assert!(validate_cross_file_assets(&assets).is_err());
    }

    #[test]
    fn validation_rejects_missing_workflow_files() {
        let mut assets = valid_assets();
        assets.retain(|asset| asset.path != "workflows/example/1.0.0/workflow.json");

        assert!(validate_cross_file_assets(&assets).is_err());
    }

    #[test]
    fn validation_rejects_duplicate_runtime_preset_identities() {
        let mut assets = valid_assets();
        assets.push(assets[0].clone());

        assert!(validate_cross_file_assets(&assets).is_err());
    }

    #[test]
    fn validation_rejects_unknown_references() {
        let mut assets = valid_assets();
        assets[7].json["schema_ref"]["id"] = json!("missing");

        assert!(validate_cross_file_assets(&assets).is_err());
    }

    #[test]
    fn validation_rejects_missing_required_input_bindings() {
        let mut assets = valid_assets();
        assets[3].json["inputs"] = json!([
            { "id": "prompt", "type": "string", "required": true },
            { "id": "style", "type": "string", "required": true }
        ]);

        assert!(validate_cross_file_assets(&assets).is_err());
    }
}
