use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;
use walkdir::WalkDir;

use super::{errors::BundledCatalogError, generated};

#[derive(Debug, Clone)]
pub struct Catalog {
    pub(crate) workflows: Vec<WorkflowEntry>,
    pub(crate) runtime_contracts: Vec<RuntimeContractEntry>,
    pub(crate) runtime_presets: Vec<RuntimePresetEntry>,
    pub(crate) execution_schemas: Vec<ExecutionSchemaEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkflowEntry {
    pub(crate) id: String,
    pub(crate) revision: String,
    pub(crate) metadata: generated::WorkflowMetadata,
    pub(crate) model_assets: generated::WorkflowModelAssets,
    pub(crate) contract_requirements: generated::WorkflowContractRequirements,
    pub(crate) execution_contract: generated::WorkflowExecutionContract,
    pub(crate) workflow_graph: generated::WorkflowGraph,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeContractEntry {
    pub(crate) id: String,
    pub(crate) revision: String,
    pub(crate) runtime_contract: generated::RuntimeContract,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePresetEntry {
    pub(crate) id: String,
    pub(crate) revision: String,
    pub(crate) runtime_preset: generated::RuntimePreset,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionSchemaEntry {
    pub(crate) id: String,
    pub(crate) revision: String,
    pub(crate) execution_schema: generated::ExecutionSchema,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogContract {
    entity: String,
    path_pattern: String,
    path_params: Vec<String>,
    required_files: Vec<RequiredFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct RequiredFile {
    name: String,
    entity: String,
    schema: String,
}

#[derive(Debug, Clone, Default)]
struct LoadedEntries {
    workflows: Vec<WorkflowEntry>,
    runtime_contracts: Vec<RuntimeContractEntry>,
    runtime_presets: Vec<RuntimePresetEntry>,
    execution_schemas: Vec<ExecutionSchemaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct ReferenceValue {
    entity: String,
    id: String,
    revision: String,
}

#[derive(Debug, Clone)]
struct InMemorySchemaRetriever {
    schemas: HashMap<String, Value>,
}

impl Retrieve for InMemorySchemaRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.schemas
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("schema not found: {uri}").into())
    }
}

impl Catalog {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, BundledCatalogError> {
        let root = root.as_ref();
        let contracts = read_contracts(root)?;
        let schemas = read_schemas(root)?;
        let mut loaded = LoadedEntries::default();

        for directory in revision_directories(root)? {
            let relative = relative_path(root, &directory);
            let contract = matching_contract(&contracts, &relative)?;
            read_revision(root, &directory, contract, &schemas, &mut loaded)?;
        }

        let catalog = loaded.into_catalog();
        resolve_references(root, &contracts, &catalog)?;
        Ok(catalog)
    }

    #[allow(dead_code)]
    pub(crate) fn workflow_revisions(&self) -> &[WorkflowEntry] {
        &self.workflows
    }

    #[allow(dead_code)]
    pub(crate) fn runtime_contract_revisions(&self) -> &[RuntimeContractEntry] {
        &self.runtime_contracts
    }

    #[allow(dead_code)]
    pub(crate) fn runtime_preset_revisions(&self) -> &[RuntimePresetEntry] {
        &self.runtime_presets
    }

    #[allow(dead_code)]
    pub(crate) fn execution_schema_revisions(&self) -> &[ExecutionSchemaEntry] {
        &self.execution_schemas
    }

    fn reference_index(&self) -> HashSet<ReferenceValue> {
        let mut loaded = HashSet::new();

        loaded.extend(self.workflows.iter().map(|entry| ReferenceValue {
            entity: "workflow_revision".to_string(),
            id: entry.id.clone(),
            revision: entry.revision.clone(),
        }));
        loaded.extend(self.runtime_contracts.iter().map(|entry| ReferenceValue {
            entity: "runtime_contract_revision".to_string(),
            id: entry.id.clone(),
            revision: entry.revision.clone(),
        }));
        loaded.extend(self.runtime_presets.iter().map(|entry| ReferenceValue {
            entity: "runtime_preset_revision".to_string(),
            id: entry.id.clone(),
            revision: entry.revision.clone(),
        }));
        loaded.extend(self.execution_schemas.iter().map(|entry| ReferenceValue {
            entity: "execution_schema_revision".to_string(),
            id: entry.id.clone(),
            revision: entry.revision.clone(),
        }));

        loaded
    }

    fn raw_values(&self) -> Result<Vec<(String, Value)>, BundledCatalogError> {
        let mut values = Vec::new();

        for entry in &self.workflows {
            let base = workflow_revision_path(&entry.id, &entry.revision);
            values.push((
                format!("{base}/metadata.json"),
                to_raw_value(format!("{base}/metadata.json"), &entry.metadata)?,
            ));
            values.push((
                format!("{base}/model_assets.json"),
                to_raw_value(format!("{base}/model_assets.json"), &entry.model_assets)?,
            ));
            values.push((
                format!("{base}/contract_requirements.json"),
                to_raw_value(
                    format!("{base}/contract_requirements.json"),
                    &entry.contract_requirements,
                )?,
            ));
            values.push((
                format!("{base}/execution_contract.json"),
                to_raw_value(
                    format!("{base}/execution_contract.json"),
                    &entry.execution_contract,
                )?,
            ));
            values.push((
                format!("{base}/workflow.json"),
                to_raw_value(format!("{base}/workflow.json"), &entry.workflow_graph)?,
            ));
        }

        for entry in &self.runtime_contracts {
            let path = format!(
                "{}/runtime_contract.json",
                runtime_contract_revision_path(&entry.id, &entry.revision)
            );
            values.push((path.clone(), to_raw_value(path, &entry.runtime_contract)?));
        }

        for entry in &self.runtime_presets {
            let path = format!(
                "{}/runtime_preset.json",
                runtime_preset_revision_path(&entry.id, &entry.revision)
            );
            values.push((path.clone(), to_raw_value(path, &entry.runtime_preset)?));
        }

        for entry in &self.execution_schemas {
            let path = format!(
                "{}/execution_schema.json",
                execution_schema_revision_path(&entry.id, &entry.revision)
            );
            values.push((path.clone(), to_raw_value(path, &entry.execution_schema)?));
        }

        Ok(values)
    }
}

impl LoadedEntries {
    fn into_catalog(self) -> Catalog {
        Catalog {
            workflows: self.workflows,
            runtime_contracts: self.runtime_contracts,
            runtime_presets: self.runtime_presets,
            execution_schemas: self.execution_schemas,
        }
    }
}

fn resolve_references(
    root: &Path,
    contracts: &[CatalogContract],
    catalog: &Catalog,
) -> Result<(), BundledCatalogError> {
    let _ = root;
    let known_contracts = contracts
        .iter()
        .map(|contract| contract.entity.as_str())
        .collect::<HashSet<_>>();
    let loaded = catalog.reference_index();

    for (path, value) in catalog.raw_values()? {
        for reference in references_in_value(&value) {
            if !known_contracts.contains(reference.entity.as_str()) {
                return Err(BundledCatalogError::Contract {
                    path,
                    message: format!("reference entity has no contract: {}", reference.entity),
                });
            }
            if !loaded.contains(&reference) {
                return Err(BundledCatalogError::UnresolvedReference {
                    path,
                    entity: reference.entity,
                    id: reference.id,
                    revision: reference.revision,
                });
            }
        }
    }

    Ok(())
}

fn references_in_value(value: &Value) -> Vec<ReferenceValue> {
    let mut references = Vec::new();
    collect_references(value, &mut references);
    references
}

fn read_contracts(root: &Path) -> Result<Vec<CatalogContract>, BundledCatalogError> {
    let contracts_root = root.join("catalog/contracts");
    let mut paths = read_dir_paths(root, &contracts_root)?;
    paths.sort();

    let mut contracts = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = relative_to_root(root, &path);
        let value = read_json_value(root, &path)?;
        let contract = serde_json::from_value::<CatalogContract>(value).map_err(|error| {
            BundledCatalogError::Contract {
                path: relative.clone(),
                message: error.to_string(),
            }
        })?;
        regress::Regex::new(&contract.path_pattern).map_err(|error| {
            BundledCatalogError::Contract {
                path: relative.clone(),
                message: error.to_string(),
            }
        })?;
        contracts.push(contract);
    }

    Ok(contracts)
}

fn read_schemas(root: &Path) -> Result<HashMap<String, Value>, BundledCatalogError> {
    let schemas_root = root.join("catalog/schemas");
    let mut paths = read_dir_paths(root, &schemas_root)?;
    paths.sort();

    let mut schemas = HashMap::with_capacity(paths.len());
    for path in paths {
        let relative = relative_to_root(root, &path);
        let value = read_json_value(root, &path)?;
        let Some(schema_id) = value.get("$id").and_then(Value::as_str).map(str::to_string) else {
            return Err(BundledCatalogError::Schema {
                path: relative,
                message: "schema is missing string $id".to_string(),
            });
        };
        if schemas.insert(schema_id.clone(), value).is_some() {
            return Err(BundledCatalogError::Schema {
                path: relative,
                message: format!("duplicate schema id: {schema_id}"),
            });
        }
    }

    Ok(schemas)
}

fn revision_directories(root: &Path) -> Result<Vec<PathBuf>, BundledCatalogError> {
    let entries_root = root.join("catalog/entries");
    let mut directories = Vec::new();

    for entry in WalkDir::new(&entries_root).min_depth(3).max_depth(3) {
        let entry = entry.map_err(|error| BundledCatalogError::Io {
            path: relative_to_root(root, &entries_root),
            message: error.to_string(),
        })?;
        if entry.file_type().is_dir() {
            directories.push(entry.into_path());
        }
    }

    directories.sort();
    Ok(directories)
}

fn relative_path(root: &Path, directory: &Path) -> String {
    relative_to_entries(root, directory)
}

fn matching_contract<'a>(
    contracts: &'a [CatalogContract],
    relative: &str,
) -> Result<&'a CatalogContract, BundledCatalogError> {
    let path = format!("catalog/entries/{relative}");
    let mut matches = contracts.iter().filter(|contract| {
        regress::Regex::new(&contract.path_pattern)
            .ok()
            .and_then(|regex| regex.find(relative))
            .is_some_and(|matched| matched.start() == 0 && matched.end() == relative.len())
    });

    let Some(contract) = matches.next() else {
        return Err(BundledCatalogError::Contract {
            path,
            message: "no matching contract".to_string(),
        });
    };

    if matches.next().is_some() {
        return Err(BundledCatalogError::Contract {
            path,
            message: "multiple matching contracts".to_string(),
        });
    }

    Ok(contract)
}

fn read_revision(
    root: &Path,
    directory: &Path,
    contract: &CatalogContract,
    schemas: &HashMap<String, Value>,
    loaded: &mut LoadedEntries,
) -> Result<(), BundledCatalogError> {
    let relative = relative_path(root, directory);
    let params = extract_path_params(contract, &relative)?;
    let id = params
        .get("id")
        .cloned()
        .ok_or_else(|| BundledCatalogError::Contract {
            path: format!("catalog/entries/{relative}"),
            message: "missing path param: id".to_string(),
        })?;
    let revision =
        params
            .get("revision")
            .cloned()
            .ok_or_else(|| BundledCatalogError::Contract {
                path: format!("catalog/entries/{relative}"),
                message: "missing path param: revision".to_string(),
            })?;

    let mut values = HashMap::with_capacity(contract.required_files.len());
    for required in &contract.required_files {
        let path = directory.join(&required.name);
        let value = read_json_value(root, &path)?;
        validate_schema(root, &path, &value, &required.schema, schemas)?;
        values.insert(required.entity.clone(), value);
    }

    match contract.entity.as_str() {
        "workflow_revision" => {
            loaded.workflows.push(WorkflowEntry {
                id,
                revision,
                metadata: parse_generated(
                    &values,
                    "workflow_metadata",
                    &workflow_revision_file_path(&relative, "metadata.json"),
                )?,
                model_assets: parse_generated(
                    &values,
                    "workflow_model_assets",
                    &workflow_revision_file_path(&relative, "model_assets.json"),
                )?,
                contract_requirements: parse_generated(
                    &values,
                    "workflow_contract_requirements",
                    &workflow_revision_file_path(&relative, "contract_requirements.json"),
                )?,
                execution_contract: parse_generated(
                    &values,
                    "workflow_execution_contract",
                    &workflow_revision_file_path(&relative, "execution_contract.json"),
                )?,
                workflow_graph: parse_generated(
                    &values,
                    "workflow_graph",
                    &workflow_revision_file_path(&relative, "workflow.json"),
                )?,
            });
        }
        "runtime_contract_revision" => {
            loaded.runtime_contracts.push(RuntimeContractEntry {
                id,
                revision,
                runtime_contract: parse_generated(
                    &values,
                    "runtime_contract",
                    &workflow_revision_file_path(&relative, "runtime_contract.json"),
                )?,
            });
        }
        "runtime_preset_revision" => {
            loaded.runtime_presets.push(RuntimePresetEntry {
                id,
                revision,
                runtime_preset: parse_generated(
                    &values,
                    "runtime_preset",
                    &workflow_revision_file_path(&relative, "runtime_preset.json"),
                )?,
            });
        }
        "execution_schema_revision" => {
            loaded.execution_schemas.push(ExecutionSchemaEntry {
                id,
                revision,
                execution_schema: parse_generated(
                    &values,
                    "execution_schema",
                    &workflow_revision_file_path(&relative, "execution_schema.json"),
                )?,
            });
        }
        _ => {
            return Err(BundledCatalogError::Contract {
                path: format!("catalog/entries/{relative}"),
                message: format!("unsupported contract entity: {}", contract.entity),
            });
        }
    }

    Ok(())
}

fn collect_references(value: &Value, references: &mut Vec<ReferenceValue>) {
    match value {
        Value::Object(map) => {
            if map.len() == 3
                && map.contains_key("entity")
                && map.contains_key("id")
                && map.contains_key("revision")
                && map.get("entity").and_then(Value::as_str).is_some()
                && map.get("id").and_then(Value::as_str).is_some()
                && map.get("revision").and_then(Value::as_str).is_some()
            {
                references.push(ReferenceValue {
                    entity: map
                        .get("entity")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    id: map
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    revision: map
                        .get("revision")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                });
            }

            for child in map.values() {
                collect_references(child, references);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_references(item, references);
            }
        }
        _ => {}
    }
}

fn extract_path_params(
    contract: &CatalogContract,
    relative: &str,
) -> Result<HashMap<String, String>, BundledCatalogError> {
    let path = format!("catalog/entries/{relative}");
    let regex = regress::Regex::new(&contract.path_pattern).map_err(|error| {
        BundledCatalogError::Contract {
            path: path.clone(),
            message: error.to_string(),
        }
    })?;
    let matched = regex
        .find(relative)
        .ok_or_else(|| BundledCatalogError::Contract {
            path: path.clone(),
            message: "path does not match contract pattern".to_string(),
        })?;

    let mut params = HashMap::with_capacity(contract.path_params.len());
    for param in &contract.path_params {
        let Some(range) = matched.named_group(param) else {
            return Err(BundledCatalogError::Contract {
                path: path.clone(),
                message: format!("missing capture group for path param: {param}"),
            });
        };
        params.insert(param.clone(), relative[range].to_string());
    }

    Ok(params)
}

fn validate_schema(
    root: &Path,
    path: &Path,
    value: &Value,
    schema_uri: &str,
    schemas: &HashMap<String, Value>,
) -> Result<(), BundledCatalogError> {
    let relative = relative_to_root(root, path);
    let Some(schema_value) = schemas.get(schema_uri) else {
        return Err(BundledCatalogError::Schema {
            path: relative,
            message: format!("schema not found: {schema_uri}"),
        });
    };

    let validator = jsonschema::options()
        .with_retriever(InMemorySchemaRetriever {
            schemas: schemas.clone(),
        })
        .build(schema_value)
        .map_err(|error| BundledCatalogError::Schema {
            path: relative.clone(),
            message: error.to_string(),
        })?;

    if let Some(error) = validator.iter_errors(value).next() {
        return Err(BundledCatalogError::Schema {
            path: relative,
            message: error.to_string(),
        });
    }

    Ok(())
}

fn parse_generated<T: serde::de::DeserializeOwned>(
    values: &HashMap<String, Value>,
    entity: &str,
    path: &str,
) -> Result<T, BundledCatalogError> {
    let value = values
        .get(entity)
        .cloned()
        .ok_or_else(|| BundledCatalogError::Contract {
            path: path.to_string(),
            message: format!("missing required value for entity: {entity}"),
        })?;

    serde_json::from_value(value).map_err(|error| BundledCatalogError::Contract {
        path: path.to_string(),
        message: error.to_string(),
    })
}

fn to_raw_value<T: serde::Serialize>(
    path: String,
    value: &T,
) -> Result<Value, BundledCatalogError> {
    serde_json::to_value(value).map_err(|error| BundledCatalogError::Contract {
        path,
        message: error.to_string(),
    })
}

fn read_json_value(root: &Path, path: &Path) -> Result<Value, BundledCatalogError> {
    let relative = relative_to_root(root, path);
    let bytes = fs::read(path).map_err(|error| BundledCatalogError::Io {
        path: relative.clone(),
        message: error.to_string(),
    })?;

    serde_json::from_slice(&bytes).map_err(|error| BundledCatalogError::JsonParse {
        path: relative,
        message: error.to_string(),
    })
}

fn read_dir_paths(root: &Path, directory: &Path) -> Result<Vec<PathBuf>, BundledCatalogError> {
    let relative = relative_to_root(root, directory);
    let mut paths = Vec::new();

    for entry in fs::read_dir(directory).map_err(|error| BundledCatalogError::Io {
        path: relative.clone(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| BundledCatalogError::Io {
            path: relative.clone(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_file() {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_to_entries(root: &Path, path: &Path) -> String {
    path.strip_prefix(root.join("catalog/entries"))
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn workflow_revision_path(id: &str, revision: &str) -> String {
    format!("catalog/entries/workflows/{id}/{revision}")
}

fn runtime_contract_revision_path(id: &str, revision: &str) -> String {
    format!("catalog/entries/runtime_contracts/{id}/{revision}")
}

fn runtime_preset_revision_path(id: &str, revision: &str) -> String {
    format!("catalog/entries/runtime_presets/{id}/{revision}")
}

fn execution_schema_revision_path(id: &str, revision: &str) -> String {
    format!("catalog/entries/execution_schemas/{id}/{revision}")
}

fn workflow_revision_file_path(relative: &str, file_name: &str) -> String {
    format!("catalog/entries/{relative}/{file_name}")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn load_reads_valid_catalog() {
        let catalog = Catalog::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
            .expect("catalog should load");

        assert_eq!(catalog.workflow_revisions().len(), 1);
        assert_eq!(catalog.runtime_contract_revisions().len(), 2);
        assert_eq!(catalog.runtime_preset_revisions().len(), 1);
        assert_eq!(catalog.execution_schema_revisions().len(), 1);
    }

    #[test]
    fn load_rejects_unresolved_reference() {
        let temp_root = copy_catalog_fixture();
        let contract_path = temp_root.join(
            "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json",
        );
        let broken = r#"{
  "contract_requirements": [
    {
      "runtime_type": "runpod",
      "endpoint_contract_ref": {
        "entity": "runtime_contract_revision",
        "id": "missing-runtime-contract",
        "revision": "1.0.0"
      },
      "provisioner_contract_ref": {
        "entity": "runtime_contract_revision",
        "id": "provisioner",
        "revision": "1.0.0"
      }
    }
  ]
}"#;
        fs::write(&contract_path, broken).expect("should update contract requirements");

        let result = Catalog::load(&temp_root);

        assert!(matches!(
            result,
            Err(BundledCatalogError::UnresolvedReference {
                path,
                entity,
                id,
                revision,
            }) if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json"
                && entity == "runtime_contract_revision"
                && id == "missing-runtime-contract"
                && revision == "1.0.0"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn references_in_value_only_collects_exact_reference_objects() {
        let value = serde_json::json!({
            "reference": {
                "entity": "runtime_contract_revision",
                "id": "provisioner",
                "revision": "1.0.0"
            },
            "not_reference": {
                "entity": "runtime_contract_revision",
                "id": "provisioner",
                "revision": "1.0.0",
                "extra": true
            },
            "nested": [
                {
                    "entity": "runtime_preset_revision",
                    "id": "comfyui-py312-cu126-torch291",
                    "revision": "1.0.0"
                }
            ]
        });

        let mut actual = references_in_value(&value);
        actual.sort_by(|left, right| {
            (&left.entity, &left.id, &left.revision).cmp(&(
                &right.entity,
                &right.id,
                &right.revision,
            ))
        });

        let mut expected = vec![
            ReferenceValue {
                entity: "runtime_contract_revision".to_string(),
                id: "provisioner".to_string(),
                revision: "1.0.0".to_string(),
            },
            ReferenceValue {
                entity: "runtime_preset_revision".to_string(),
                id: "comfyui-py312-cu126-torch291".to_string(),
                revision: "1.0.0".to_string(),
            },
        ];
        expected.sort_by(|left, right| {
            (&left.entity, &left.id, &left.revision).cmp(&(
                &right.entity,
                &right.id,
                &right.revision,
            ))
        });

        assert_eq!(actual, expected);
    }

    fn copy_catalog_fixture() -> PathBuf {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled");
        let temp_root = std::env::temp_dir().join(format!(
            "luma-forge-bundled-catalog-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));
        copy_dir_all(&fixture_root, &temp_root);
        temp_root
    }

    fn copy_dir_all(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("should create temp root");

        for entry in walkdir::WalkDir::new(from) {
            let entry = entry.expect("walkdir entry should exist");
            let relative = entry
                .path()
                .strip_prefix(from)
                .expect("entry should stay under source root");
            let target = to.join(relative);

            if entry.file_type().is_dir() {
                fs::create_dir_all(&target).expect("should create temp directory");
                continue;
            }

            fs::copy(entry.path(), &target).expect("should copy fixture file");
        }
    }
}
