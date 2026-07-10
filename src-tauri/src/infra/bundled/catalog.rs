use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use jsonschema::{Retrieve, Uri};
use serde::Deserialize;
use serde_json::Value;
use tokio::fs;
use walkdir::WalkDir;

use super::{entries::CatalogEntry, errors::BundledCatalogError};

#[derive(Debug)]
pub struct Catalog {
    root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CatalogContract {
    entity: String,
    path_pattern: String,
    required_files: Vec<RequiredFile>,
}

#[derive(Debug, Deserialize)]
struct RequiredFile {
    name: String,
    schema: String,
}

#[derive(Debug)]
struct RevisionDescriptor {
    contract_index: usize,
    entity: String,
    id: String,
    revision: String,
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct ReferenceValue {
    entity: String,
    id: String,
    revision: String,
}

#[derive(Debug)]
struct LocatedReference {
    path: String,
    value: ReferenceValue,
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

pub(super) struct Query {
    root: PathBuf,
    contracts: Vec<CatalogContract>,
    schemas: HashMap<String, Value>,
    revisions: Vec<RevisionDescriptor>,
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(super) async fn query(&self) -> Result<Query, BundledCatalogError> {
        let contracts = read_contracts(&self.root).await?;
        let schemas = read_schemas(&self.root).await?;
        let revisions = build_revision_index(&self.root, &contracts)?;

        Ok(Query {
            root: self.root.clone(),
            contracts,
            schemas,
            revisions,
        })
    }
}

impl Query {
    pub(super) async fn load<E: CatalogEntry>(
        &self,
        key: Option<&(String, String)>,
        first_only: bool,
    ) -> Result<Vec<E>, BundledCatalogError> {
        if !self
            .contracts
            .iter()
            .any(|contract| contract.entity == E::ENTITY)
        {
            return Err(BundledCatalogError::Contract {
                path: "catalog/contracts".to_string(),
                message: format!("entry entity has no contract: {}", E::ENTITY),
            });
        }

        let reference_index = self
            .revisions
            .iter()
            .map(|descriptor| ReferenceValue {
                entity: descriptor.entity.clone(),
                id: descriptor.id.clone(),
                revision: descriptor.revision.clone(),
            })
            .collect::<HashSet<_>>();
        let mut entries = Vec::new();

        for descriptor in self.revisions.iter().filter(|descriptor| {
            descriptor.entity == E::ENTITY
                && key.is_none_or(|(id, revision)| {
                    descriptor.id == *id && descriptor.revision == *revision
                })
        }) {
            entries.push(self.read_entry::<E>(descriptor, &reference_index).await?);
            if first_only {
                break;
            }
        }

        Ok(entries)
    }

    async fn read_entry<E: CatalogEntry>(
        &self,
        descriptor: &RevisionDescriptor,
        reference_index: &HashSet<ReferenceValue>,
    ) -> Result<E, BundledCatalogError> {
        let contract = &self.contracts[descriptor.contract_index];
        let relative = relative_to_entries(&self.root, &descriptor.path);
        let mut documents = HashMap::with_capacity(contract.required_files.len());
        let mut references = Vec::new();

        for required in &contract.required_files {
            let path = descriptor.path.join(&required.name);
            match fs::metadata(&path).await {
                Ok(metadata) if metadata.is_file() => {}
                Ok(_) => {
                    return Err(BundledCatalogError::Contract {
                        path: relative_to_root(&self.root, &path),
                        message: "missing required file".to_string(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(BundledCatalogError::Contract {
                        path: relative_to_root(&self.root, &path),
                        message: "missing required file".to_string(),
                    });
                }
                Err(error) => {
                    return Err(BundledCatalogError::Io {
                        path: relative_to_root(&self.root, &path),
                        message: error.to_string(),
                    });
                }
            }

            let value = read_json_value(&self.root, &path).await?;
            validate_schema(&self.root, &path, &value, &required.schema, &self.schemas)?;
            let reference_path = relative_to_root(&self.root, &path);
            references.extend(references_in_value(&value).into_iter().map(|value| {
                LocatedReference {
                    path: reference_path.clone(),
                    value,
                }
            }));
            documents.insert(required.name.clone(), value);
        }

        resolve_references(&self.contracts, reference_index, references)?;
        E::from_documents(
            descriptor.id.clone(),
            descriptor.revision.clone(),
            relative,
            documents,
        )
    }
}

fn resolve_references(
    contracts: &[CatalogContract],
    reference_index: &HashSet<ReferenceValue>,
    references: Vec<LocatedReference>,
) -> Result<(), BundledCatalogError> {
    for LocatedReference { path, value } in references {
        if !contracts
            .iter()
            .any(|contract| contract.entity == value.entity)
        {
            return Err(BundledCatalogError::Contract {
                path,
                message: format!("reference entity has no contract: {}", value.entity),
            });
        }
        if !reference_index.contains(&value) {
            return Err(BundledCatalogError::UnresolvedReference {
                path,
                entity: value.entity,
                id: value.id,
                revision: value.revision,
            });
        }
    }

    Ok(())
}

fn references_in_value(value: &Value) -> Vec<ReferenceValue> {
    let mut references = Vec::new();
    collect_references(value, &mut references);
    references
}

async fn read_contracts(root: &Path) -> Result<Vec<CatalogContract>, BundledCatalogError> {
    let contracts_root = root.join("catalog/contracts");
    let mut paths = read_dir_paths(root, &contracts_root).await?;
    paths.sort();

    let mut contracts = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = relative_to_root(root, &path);
        let value = read_json_value(root, &path).await?;
        let contract = serde_json::from_value::<CatalogContract>(value).map_err(|error| {
            BundledCatalogError::Contract {
                path: relative.clone(),
                message: error.to_string(),
            }
        })?;
        regress::Regex::new(&contract.path_pattern).map_err(|error| {
            BundledCatalogError::Contract {
                path: relative,
                message: error.to_string(),
            }
        })?;
        contracts.push(contract);
    }

    Ok(contracts)
}

async fn read_schemas(root: &Path) -> Result<HashMap<String, Value>, BundledCatalogError> {
    let schemas_root = root.join("catalog/schemas");
    let mut paths = read_dir_paths(root, &schemas_root).await?;
    paths.sort();

    let mut schemas = HashMap::with_capacity(paths.len());
    for path in paths {
        let relative = relative_to_root(root, &path);
        let value = read_json_value(root, &path).await?;
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

fn build_revision_index(
    root: &Path,
    contracts: &[CatalogContract],
) -> Result<Vec<RevisionDescriptor>, BundledCatalogError> {
    revision_directories(root)?
        .into_iter()
        .map(|path| {
            let relative = relative_to_entries(root, &path);
            let contract_index = matching_contract(contracts, &relative)?;
            let contract = &contracts[contract_index];
            let (id, revision) = extract_path_identity(contract, &relative)?;

            Ok(RevisionDescriptor {
                contract_index,
                entity: contract.entity.clone(),
                id,
                revision,
                path,
            })
        })
        .collect()
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

fn matching_contract(
    contracts: &[CatalogContract],
    relative: &str,
) -> Result<usize, BundledCatalogError> {
    let path = format!("catalog/entries/{relative}");
    let mut matches = contracts.iter().enumerate().filter(|(_, contract)| {
        regress::Regex::new(&contract.path_pattern)
            .ok()
            .and_then(|regex| regex.find(relative))
            .is_some_and(|matched| matched.start() == 0 && matched.end() == relative.len())
    });

    let Some((index, _)) = matches.next() else {
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

    Ok(index)
}

fn collect_references(value: &Value, references: &mut Vec<ReferenceValue>) {
    match value {
        Value::Object(map) => {
            if let (3, Some(entity), Some(id), Some(revision)) = (
                map.len(),
                map.get("entity").and_then(Value::as_str),
                map.get("id").and_then(Value::as_str),
                map.get("revision").and_then(Value::as_str),
            ) {
                references.push(ReferenceValue {
                    entity: entity.to_string(),
                    id: id.to_string(),
                    revision: revision.to_string(),
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

fn extract_path_identity(
    contract: &CatalogContract,
    relative: &str,
) -> Result<(String, String), BundledCatalogError> {
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

    let extract = |name| {
        matched
            .named_group(name)
            .map(|range| relative[range].to_string())
            .ok_or_else(|| BundledCatalogError::Contract {
                path: path.clone(),
                message: format!("missing named capture group: {name}"),
            })
    };

    Ok((extract("id")?, extract("revision")?))
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

async fn read_json_value(root: &Path, path: &Path) -> Result<Value, BundledCatalogError> {
    let relative = relative_to_root(root, path);
    let bytes = fs::read(path)
        .await
        .map_err(|error| BundledCatalogError::Io {
            path: relative.clone(),
            message: error.to_string(),
        })?;

    serde_json::from_slice(&bytes).map_err(|error| BundledCatalogError::JsonParse {
        path: relative,
        message: error.to_string(),
    })
}

async fn read_dir_paths(
    root: &Path,
    directory: &Path,
) -> Result<Vec<PathBuf>, BundledCatalogError> {
    let relative = relative_to_root(root, directory);
    let mut paths = Vec::new();
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|error| BundledCatalogError::Io {
            path: relative.clone(),
            message: error.to_string(),
        })?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| BundledCatalogError::Io {
            path: relative.clone(),
            message: error.to_string(),
        })?
    {
        if entry
            .file_type()
            .await
            .map_err(|error| BundledCatalogError::Io {
                path: relative.clone(),
                message: error.to_string(),
            })?
            .is_file()
        {
            paths.push(entry.path());
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::infra::bundled::entries::workflows;

    static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn new_performs_no_io() {
        let _catalog = Catalog::new("missing-bundled-root");
    }

    #[tokio::test]
    async fn query_rejects_unresolved_reference() {
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

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

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

    #[tokio::test]
    async fn query_rejects_unknown_reference_entity() {
        let temp_root = copy_catalog_fixture();
        let path = temp_root.join(
            "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json",
        );
        let value = fs::read_to_string(&path)
            .expect("should read contract requirements")
            .replace("runtime_contract_revision", "missing_revision");
        fs::write(&path, value).expect("should update contract requirements");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::Contract { path, message })
                if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json"
                    && message == "reference entity has no contract: missing_revision"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn query_rejects_invalid_json() {
        let temp_root = copy_catalog_fixture();
        let path =
            temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json");
        fs::write(&path, "{").expect("should break metadata JSON");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::JsonParse { path, .. })
                if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn query_rejects_schema_violation() {
        let temp_root = copy_catalog_fixture();
        let path =
            temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json");
        fs::write(&path, "{}").expect("should replace metadata");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::Schema { path, .. })
                if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn query_rejects_unknown_schema() {
        let temp_root = copy_catalog_fixture();
        let path = temp_root.join("catalog/contracts/workflow_revision.json");
        let value = fs::read_to_string(&path)
            .expect("should read workflow contract")
            .replace(
                "luma-forge://schema/workflow_metadata",
                "luma-forge://schema/missing",
            );
        fs::write(&path, value).expect("should update workflow contract");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::Schema { path, message })
                if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json"
                    && message == "schema not found: luma-forge://schema/missing"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn query_rejects_entry_without_matching_contract() {
        let temp_root = copy_catalog_fixture();
        let path = temp_root.join("catalog/entries/unknown/item/1.0.0");
        fs::create_dir_all(&path).expect("should add unmatched entry");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::Contract { path, message })
                if path == "catalog/entries/unknown/item/1.0.0"
                    && message == "no matching contract"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn query_rejects_missing_required_file() {
        let temp_root = copy_catalog_fixture();
        let missing_path =
            temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json");
        fs::remove_file(&missing_path).expect("should remove required file");

        let catalog = Catalog::new(&temp_root);
        let result = workflows::Entry::find().all(&catalog).await;

        assert!(matches!(
            result,
            Err(BundledCatalogError::Contract { path, message })
                if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json"
                    && message == "missing required file"
        ));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[tokio::test]
    async fn find_by_id_reads_only_selected_entry() {
        let temp_root = copy_catalog_fixture();
        let entries = temp_root.join("catalog/entries/workflows");
        copy_dir_all(
            &entries.join("comfyui-hidream-o1-dev/1.0.0"),
            &entries.join("selected/1.0.0"),
        );
        fs::write(
            entries.join("comfyui-hidream-o1-dev/1.0.0/metadata.json"),
            "{",
        )
        .expect("should break unselected metadata");

        let catalog = Catalog::new(&temp_root);
        let entry = workflows::Entry::find_by_id(("selected", "1.0.0"))
            .one(&catalog)
            .await
            .expect("selected query should succeed")
            .expect("selected workflow should exist");

        assert_eq!(entry.id, "selected");
        let _ = fs::remove_dir_all(temp_root);
    }

    fn copy_catalog_fixture() -> PathBuf {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled");
        let temp_root = std::env::temp_dir().join(format!(
            "luma-forge-bundled-catalog-{}-{}",
            std::process::id(),
            NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
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
