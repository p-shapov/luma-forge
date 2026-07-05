use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::validation_errors::BundledValidationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledJsonFile {
    pub path: String,
    pub schema_id: String,
    pub path_params: BTreeMap<String, String>,
    pub json: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDocument {
    pub id: String,
    pub json: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LayoutSpec {
    pub path: String,
    pub entity: String,
    pub path_pattern: String,
    pub path_params: Vec<String>,
    pub files: BTreeMap<String, LayoutFileSpec>,
}

#[derive(Debug, Clone)]
pub struct LayoutFileSpec {
    pub schema: String,
}

impl LayoutSpec {
    pub fn from_json(path: &str, json: serde_json::Value) -> Result<Self, BundledValidationError> {
        let entity = json
            .get("entity")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid(path, "layout missing entity"))?
            .to_string();
        let path_pattern = json
            .get("path_pattern")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid(path, "layout missing path_pattern"))?
            .to_string();
        let path_params = json
            .get("path_params")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid(path, "layout missing path_params"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| invalid(path, "layout path_params must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let files = json
            .get("files")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| invalid(path, "layout missing files"))?
            .iter()
            .map(|(name, file)| {
                let schema = file
                    .get("schema")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| invalid(path, "layout file missing schema"))?
                    .to_string();
                Ok((name.clone(), LayoutFileSpec { schema }))
            })
            .collect::<Result<BTreeMap<_, _>, BundledValidationError>>()?;
        Ok(Self {
            path: path.to_string(),
            entity,
            path_pattern,
            path_params,
            files,
        })
    }
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

pub fn validate_cross_file_assets(
    assets: &[BundledJsonFile],
) -> Result<(), BundledValidationError> {
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
            ["workflows", ..] => {
                let workflow_id = asset_id(asset)?.to_string();
                let revision = asset_revision(asset)?.to_string();
                let file = asset_file(asset)
                    .ok_or_else(|| invalid(&asset.path, "workflow file missing path file"))?;
                let files = workflow_files
                    .entry((workflow_id, revision))
                    .or_default();
                if !files.insert(file.to_string()) {
                    return Err(invalid(&asset.path, "duplicate workflow file identity"));
                }
            }
            ["runtime_presets", ..] | ["runtime_contracts", ..] | ["execution_schemas", ..] => {
                let key = (
                    asset_id(asset)?.to_string(),
                    asset_revision(asset)?.to_string(),
                );
                match asset.schema_id.as_str() {
                    "luma-forge://schemas/bundled/runtime_preset.schema.json" => {
                        if !runtime_presets.insert(key) {
                            return Err(invalid(&asset.path, "duplicate runtime preset identity"));
                        }
                    }
                    "luma-forge://schemas/bundled/runtime_contract.schema.json" => {
                        if !runtime_contracts.insert(key) {
                            return Err(invalid(
                                &asset.path,
                                "duplicate runtime contract identity",
                            ));
                        }
                    }
                    "luma-forge://schemas/bundled/execution_schema.schema.json" => {
                        if !execution_schemas.insert(key.clone()) {
                            return Err(invalid(
                                &asset.path,
                                "duplicate execution schema identity",
                            ));
                        }
                        execution_schema_inputs.insert(key, execution_input_ids(asset));
                    }
                    _ => {}
                }
            }
            _ => return Err(invalid(&asset.path, "unexpected bundled JSON path")),
        }
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

fn asset_id(asset: &BundledJsonFile) -> Result<&str, BundledValidationError> {
    asset
        .path_params
        .get("id")
        .map(String::as_str)
        .ok_or_else(|| invalid(&asset.path, "bundled path missing id"))
}

fn asset_revision(asset: &BundledJsonFile) -> Result<&str, BundledValidationError> {
    asset
        .path_params
        .get("revision")
        .map(String::as_str)
        .ok_or_else(|| invalid(&asset.path, "bundled path missing revision"))
}

fn asset_file(asset: &BundledJsonFile) -> Option<&str> {
    asset.path_params.get("file").map(String::as_str)
}

fn reject_unsafe_model_paths(asset: &BundledJsonFile) -> Result<(), BundledValidationError> {
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

fn execution_input_ids(asset: &BundledJsonFile) -> BTreeMap<String, bool> {
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
    asset: &BundledJsonFile,
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
    layouts: &[LayoutSpec],
) -> Result<Vec<BundledJsonFile>, BundledValidationError> {
    let mut files = Vec::new();

    for path in sorted_json_files(root)? {
        let relative = path
            .strip_prefix(root)
            .expect("bundled path should be under bundled root")
            .to_string_lossy()
            .replace('\\', "/");

        let text = std::fs::read_to_string(&path)
            .map_err(|error| invalid(&relative, format!("bundled read failed: {error}")))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| invalid(&relative, format!("bundled JSON parse failed: {error}")))?;
        let (layout, path_params) = match_layout(&relative, layouts)?;
        let file_key = path_params
            .get("file")
            .map(String::as_str)
            .unwrap_or("__self__");
        let schema_id = json
            .get("$schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid(&relative, "bundled JSON missing $schema"))?;
        let expected_schema = layout
            .files
            .get(file_key)
            .ok_or_else(|| invalid(&relative, "unexpected bundled JSON path"))?
            .schema
            .as_str();
        if schema_id != expected_schema {
            return Err(invalid(&relative, "bundled schema does not match layout"));
        }
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

        files.push(BundledJsonFile {
            path: relative,
            schema_id: schema_id.to_string(),
            path_params,
            json,
        });
    }

    Ok(files)
}

fn sorted_json_files(root: &Path) -> Result<Vec<PathBuf>, BundledValidationError> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1).sort_by_file_name() {
        let entry = entry.map_err(|error| {
            let path = error
                .path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| root.display().to_string());
            invalid(
                &path,
                format!("bundled directory traversal failed: {error}"),
            )
        })?;
        if entry.file_type().is_dir() {
            continue;
        }
        if entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("json")
        {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            return Err(invalid(&relative, "unexpected bundled non-JSON path"));
        }
        files.push(entry.into_path());
    }
    Ok(files)
}

fn match_layout<'a>(
    relative: &str,
    layouts: &'a [LayoutSpec],
) -> Result<(&'a LayoutSpec, BTreeMap<String, String>), BundledValidationError> {
    let mut matched = None;
    for layout in layouts {
        let regex = regress::Regex::new(&layout.path_pattern).map_err(|error| {
            invalid(&layout.path, format!("layout path pattern failed: {error}"))
        })?;
        let Some(captures) = regex.find(relative) else {
            continue;
        };
        if captures.range() != (0..relative.len()) {
            continue;
        }
        if matched.is_some() {
            return Err(invalid(relative, "bundled path matches multiple layouts"));
        }
        let mut path_params = BTreeMap::new();
        for param in &layout.path_params {
            let Some(range) = captures.named_group(param) else {
                return Err(invalid(
                    &layout.path,
                    format!("layout missing capture {param}"),
                ));
            };
            path_params.insert(param.clone(), relative[range].to_string());
        }
        matched = Some((layout, path_params));
    }
    matched.ok_or_else(|| invalid(relative, "unexpected bundled JSON path"))
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
    fn validate_bundled_catalog_keeps_layout_path_captures() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "runtime_presets/base/1.0.0.json",
            r#"{
              "$schema": "luma-forge://schemas/bundled/runtime_preset.schema.json",
              "runtime": {}
            }"#,
        );

        let files =
            validate_bundled_catalog(fixture.path(), &test_schemas(), &test_layouts()).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "runtime_presets/base/1.0.0.json");
        assert_eq!(
            files[0].path_params.get("id").map(String::as_str),
            Some("base")
        );
        assert_eq!(
            files[0].path_params.get("revision").map(String::as_str),
            Some("1.0.0")
        );
    }

    #[test]
    fn validate_bundled_catalog_rejects_schema_that_does_not_match_layout() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "runtime_presets/base/1.0.0.json",
            r#"{
              "$schema": "luma-forge://schemas/bundled/workflow_metadata.schema.json",
              "name": "Wrong"
            }"#,
        );

        let error =
            validate_bundled_catalog(fixture.path(), &test_schemas(), &test_layouts()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, "runtime_presets/base/1.0.0.json");
        assert_eq!(message, "bundled schema does not match layout");
    }

    #[test]
    fn validate_bundled_catalog_returns_err_for_invalid_json() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/metadata.json",
            r#"{"$schema":"luma-forge://schemas/bundled/workflow_metadata.schema.json""#,
        );

        let error =
            validate_bundled_catalog(fixture.path(), &test_schemas(), &test_layouts()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, "workflows/example-flow/1.0.0/metadata.json");
        assert!(message.starts_with("bundled JSON parse failed:"));
    }

    #[test]
    fn validate_bundled_catalog_returns_err_for_missing_root() {
        let fixture = TestFixture::new();
        let root = fixture.path().to_path_buf();
        drop(fixture);

        let error = validate_bundled_catalog(&root, &test_schemas(), &test_layouts()).unwrap_err();

        let BundledValidationError::Invalid { path, message } = error;
        assert_eq!(path, root.display().to_string());
        assert!(message.starts_with("bundled directory traversal failed:"));
    }

    #[test]
    fn validate_bundled_catalog_uses_json_schema_as_source_of_truth() {
        let fixture = TestFixture::new();
        fixture.write_json(
            "workflows/example-flow/1.0.0/model_assets.json",
            r#"{
              "$schema": "luma-forge://schemas/bundled/workflow_model_assets.schema.json",
              "model_assets": []
            }"#,
        );

        let assets =
            validate_bundled_catalog(fixture.path(), &test_schemas(), &test_layouts()).unwrap();

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
            SchemaDocument {
                id: "luma-forge://schemas/bundled/runtime_preset.schema.json".to_string(),
                json: serde_json::json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "$id": "luma-forge://schemas/bundled/runtime_preset.schema.json",
                    "type": "object",
                    "required": ["$schema", "runtime"],
                    "properties": {
                        "$schema": {
                            "const": "luma-forge://schemas/bundled/runtime_preset.schema.json"
                        },
                        "runtime": { "type": "object" }
                    },
                    "additionalProperties": false
                }),
            },
        ]
    }

    fn test_layouts() -> Vec<LayoutSpec> {
        vec![
            LayoutSpec::from_json(
                "schemas/bundled/layouts/runtime_preset.layout.json",
                serde_json::json!({
                    "entity": "runtime_preset",
                    "path_pattern": "^runtime_presets/(?<id>[a-z0-9][a-z0-9_-]*)/(?<revision>[0-9]+\\.[0-9]+\\.[0-9]+)\\.json$",
                    "path_params": ["id", "revision"],
                    "files": {
                        "__self__": {
                            "schema": "luma-forge://schemas/bundled/runtime_preset.schema.json"
                        }
                    }
                }),
            )
            .unwrap(),
            LayoutSpec::from_json(
                "schemas/bundled/layouts/workflow_revision.layout.json",
                serde_json::json!({
                    "entity": "workflow_revision",
                    "path_pattern": "^workflows/(?<id>[a-z0-9][a-z0-9_-]*)/(?<revision>[0-9]+\\.[0-9]+\\.[0-9]+)/(?<file>[a-z_]+\\.json)$",
                    "path_params": ["id", "revision", "file"],
                    "files": {
                        "metadata.json": {
                            "schema": "luma-forge://schemas/bundled/workflow_metadata.schema.json"
                        },
                        "model_assets.json": {
                            "schema": "luma-forge://schemas/bundled/workflow_model_assets.schema.json"
                        }
                    }
                }),
            )
            .unwrap(),
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

    fn path_params(path: &str) -> BTreeMap<String, String> {
        let parts: Vec<&str> = path.split('/').collect();
        let mut params = BTreeMap::new();
        match parts.as_slice() {
            ["workflows", id, revision, file] => {
                params.insert("id".to_string(), (*id).to_string());
                params.insert("revision".to_string(), (*revision).to_string());
                params.insert("file".to_string(), (*file).to_string());
            }
            ["runtime_presets", id, file]
            | ["runtime_contracts", id, file]
            | ["execution_schemas", id, file] => {
                params.insert("id".to_string(), (*id).to_string());
                params.insert(
                    "revision".to_string(),
                    file.trim_end_matches(".json").to_string(),
                );
            }
            _ => {}
        }
        params
    }

    fn asset(path: &str, schema_id: &str, json: serde_json::Value) -> BundledJsonFile {
        BundledJsonFile {
            path: path.to_string(),
            schema_id: schema_id.to_string(),
            path_params: path_params(path),
            json,
        }
    }

    fn valid_assets() -> Vec<BundledJsonFile> {
        vec![
            asset(
                "runtime_presets/base/1.0.0.json",
                "luma-forge://schemas/bundled/runtime_preset.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/runtime_preset.schema.json",
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
                    "image_ref": "ghcr.io/example/endpoint:latest"
                }),
            ),
            asset(
                "runtime_contracts/provisioner/1.0.0.json",
                "luma-forge://schemas/bundled/runtime_contract.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/runtime_contract.schema.json",
                    "image_ref": "ghcr.io/example/provisioner:latest"
                }),
            ),
            asset(
                "execution_schemas/text-to-image/1.0.0.json",
                "luma-forge://schemas/bundled/execution_schema.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/execution_schema.schema.json",
                    "inputs": [{ "id": "prompt", "type": "string", "required": true }],
                    "outputs": { "type": "image_set" }
                }),
            ),
            asset(
                "workflows/example/1.0.0/metadata.json",
                "luma-forge://schemas/bundled/workflow_metadata.schema.json",
                json!({
                    "$schema": "luma-forge://schemas/bundled/workflow_metadata.schema.json",
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
    fn validation_uses_path_params_for_runtime_preset_identity() {
        let mut assets = valid_assets();
        assets[0].path_params.insert("id".to_string(), "other-base".to_string());

        assert_eq!(
            validate_cross_file_assets(&assets),
            Err(BundledValidationError::Invalid {
                path: "workflows/example/1.0.0/metadata.json".to_string(),
                message: "workflow metadata references an unknown runtime preset".to_string(),
            })
        );
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
    fn validation_rejects_runtime_contract_disguised_as_runtime_preset_path() {
        let mut assets = valid_assets();
        assets[0].schema_id =
            "luma-forge://schemas/bundled/runtime_contract.schema.json".to_string();
        assets[0].json = json!({
            "$schema": "luma-forge://schemas/bundled/runtime_contract.schema.json",
            "image_ref": "ghcr.io/example/not-a-preset:latest"
        });

        assert_eq!(
            validate_cross_file_assets(&assets),
            Err(BundledValidationError::Invalid {
                path: "workflows/example/1.0.0/metadata.json".to_string(),
                message: "workflow metadata references an unknown runtime preset".to_string(),
            })
        );
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
