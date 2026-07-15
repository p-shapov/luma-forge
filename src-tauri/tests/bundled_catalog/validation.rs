use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs as stdfs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

use serde::Deserialize;
use serde_json::Value;
use tokio::fs;

#[derive(Debug, thiserror::Error)]
pub(super) enum ValidationError {
    #[error("bundled catalog io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("bundled catalog json error at {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("bundled catalog contract error at {path}: {message}")]
    Contract { path: String, message: String },
    #[error("bundled catalog schema error at {path}: {message}")]
    Schema { path: String, message: String },
    #[error("bundled catalog unresolved reference at {path}: {contract}/{id}/{revision}")]
    UnresolvedReference {
        path: String,
        contract: String,
        id: String,
        revision: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogContract {
    entries_path: String,
    required_files: Vec<RequiredFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiredFile {
    name: String,
    schema: String,
}

struct RevisionDescriptor {
    id: String,
    revision: String,
    path: PathBuf,
}

#[derive(Clone)]
struct SchemaRetriever {
    values: HashMap<String, Value>,
}

impl jsonschema::Retrieve for SchemaRetriever {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.values
            .get(uri.as_str())
            .cloned()
            .ok_or_else(|| format!("schema not found: {uri}").into())
    }
}

struct Schemas {
    values: HashMap<String, Value>,
    validators: HashMap<String, jsonschema::Validator>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize)]
struct ReferenceValue {
    contract: String,
    id: String,
    revision: String,
}

struct LocatedReference {
    path: String,
    value: ReferenceValue,
}

struct LocatedContract {
    path: String,
    contract: CatalogContract,
}

struct AuditDescriptor {
    contract_index: usize,
    id: String,
    revision: String,
    path: PathBuf,
}

impl Schemas {
    async fn load(root: &Path) -> Result<Self, ValidationError> {
        let files = read_direct_files(root, &root.join("catalog/schemas")).await?;
        let mut values = HashMap::with_capacity(files.len());
        let mut schemas = Vec::with_capacity(files.len());

        for path in files {
            let relative = relative_to_root(root, &path);
            let name = path
                .file_name()
                .and_then(OsStr::to_str)
                .filter(|name| is_safe_component(name))
                .ok_or_else(|| ValidationError::Schema {
                    path: relative.clone(),
                    message: "schema filename must be a safe UTF-8 component".to_string(),
                })?;
            let value = read_json_value(root, &path).await?;
            let expected_id = format!("luma-forge://schema/{name}");
            let id = value
                .get("$id")
                .and_then(Value::as_str)
                .ok_or_else(|| ValidationError::Schema {
                    path: relative.clone(),
                    message: "schema must have a string $id".to_string(),
                })?
                .to_string();
            if id != expected_id {
                return Err(ValidationError::Schema {
                    path: relative,
                    message: format!("schema $id must be {expected_id}"),
                });
            }
            if values.insert(id.clone(), value).is_some() {
                return Err(ValidationError::Schema {
                    path: relative,
                    message: format!("duplicate schema $id: {id}"),
                });
            }
            schemas.push((id, relative));
        }

        let retriever = SchemaRetriever {
            values: values.clone(),
        };
        let mut validators = HashMap::with_capacity(values.len());
        for (id, path) in schemas {
            let validator = jsonschema::options()
                .with_retriever(retriever.clone())
                .build(&values[&id])
                .map_err(|error| ValidationError::Schema {
                    path,
                    message: error.to_string(),
                })?;
            validators.insert(id, validator);
        }

        Ok(Self { values, validators })
    }
}

pub(super) async fn validate(root: &Path) -> Result<(), ValidationError> {
    let contracts = read_all_contracts(root).await?;
    let schemas = Schemas::load(root).await?;

    for located in &contracts {
        for required in &located.contract.required_files {
            if !schemas.values.contains_key(&required.schema) {
                return Err(ValidationError::Schema {
                    path: located.path.clone(),
                    message: format!("schema not found: {}", required.schema),
                });
            }
        }
    }

    let descriptors = audit_descriptors(root, &contracts).await?;
    let index = descriptors
        .iter()
        .map(|descriptor| ReferenceValue {
            contract: contracts[descriptor.contract_index].path.clone(),
            id: descriptor.id.clone(),
            revision: descriptor.revision.clone(),
        })
        .collect::<HashSet<_>>();
    let known_contracts = contracts
        .iter()
        .map(|contract| contract.path.as_str())
        .collect::<HashSet<_>>();

    for descriptor in &descriptors {
        let located = &contracts[descriptor.contract_index];
        let references =
            read_validated_references(root, descriptor, &located.contract, &schemas).await?;
        validate_references(references, &known_contracts, &index)?;
    }

    Ok(())
}

async fn read_all_contracts(root: &Path) -> Result<Vec<LocatedContract>, ValidationError> {
    let files = read_direct_files(root, &root.join("catalog/contracts")).await?;
    let mut contracts = Vec::with_capacity(files.len());
    let mut entries_paths = HashSet::with_capacity(files.len());

    for path in files {
        let contract_path = contract_identity(root, &path)?;
        let value = read_json_value(root, &path).await?;
        let contract = serde_json::from_value::<CatalogContract>(value).map_err(|error| {
            ValidationError::Contract {
                path: contract_path.clone(),
                message: error.to_string(),
            }
        })?;
        validate_contract(&contract, &contract_path)?;
        if !entries_paths.insert(contract.entries_path.clone()) {
            return Err(ValidationError::Contract {
                path: contract_path,
                message: format!("duplicate entries_path: {}", contract.entries_path),
            });
        }
        contracts.push(LocatedContract {
            path: contract_path,
            contract,
        });
    }

    Ok(contracts)
}

fn contract_identity(root: &Path, path: &Path) -> Result<String, ValidationError> {
    let relative = relative_to_root(root, path);
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| is_safe_component(name))
        .ok_or_else(|| ValidationError::Contract {
            path: relative,
            message: "contract filename must be a safe UTF-8 component".to_string(),
        })?;
    Ok(format!("catalog/contracts/{name}"))
}

async fn audit_descriptors(
    root: &Path,
    contracts: &[LocatedContract],
) -> Result<Vec<AuditDescriptor>, ValidationError> {
    let mut descriptors = Vec::new();
    for (contract_index, located) in contracts.iter().enumerate() {
        for RevisionDescriptor { id, revision, path } in
            revision_descriptors(root, &located.contract).await?
        {
            descriptors.push(AuditDescriptor {
                contract_index,
                id,
                revision,
                path,
            });
        }
    }
    Ok(descriptors)
}

async fn revision_descriptors(
    root: &Path,
    contract: &CatalogContract,
) -> Result<Vec<RevisionDescriptor>, ValidationError> {
    let entries_root = root.join(&contract.entries_path);
    let mut descriptors = Vec::new();

    for (id, id_path) in read_directories(root, &entries_root).await? {
        for (revision, path) in read_directories(root, &id_path).await? {
            descriptors.push(RevisionDescriptor {
                id: id.clone(),
                revision,
                path,
            });
        }
    }

    descriptors
        .sort_by(|left, right| (&left.id, &left.revision).cmp(&(&right.id, &right.revision)));
    Ok(descriptors)
}

async fn read_validated_references(
    root: &Path,
    descriptor: &AuditDescriptor,
    contract: &CatalogContract,
    schemas: &Schemas,
) -> Result<Vec<LocatedReference>, ValidationError> {
    let mut references = Vec::new();
    for required in &contract.required_files {
        let path = descriptor.path.join(&required.name);
        let relative = relative_to_root(root, &path);
        let value = read_required_json(root, &path).await?;
        let validator = &schemas.validators[&required.schema];
        validator
            .validate(&value)
            .map_err(|error| ValidationError::Schema {
                path: relative.clone(),
                message: error.to_string(),
            })?;

        let mut document_references = Vec::new();
        collect_references(&value, &mut document_references);
        references.extend(
            document_references
                .into_iter()
                .map(|value| LocatedReference {
                    path: relative.clone(),
                    value,
                }),
        );
    }
    Ok(references)
}

fn collect_references(value: &Value, output: &mut Vec<ReferenceValue>) {
    match value {
        Value::Object(map) => {
            if let (3, Some(contract), Some(id), Some(revision)) = (
                map.len(),
                map.get("contract").and_then(Value::as_str),
                map.get("id").and_then(Value::as_str),
                map.get("revision").and_then(Value::as_str),
            ) {
                output.push(ReferenceValue {
                    contract: contract.to_string(),
                    id: id.to_string(),
                    revision: revision.to_string(),
                });
            }
            for child in map.values() {
                collect_references(child, output);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_references(item, output);
            }
        }
        _ => {}
    }
}

fn validate_references(
    references: Vec<LocatedReference>,
    known_contracts: &HashSet<&str>,
    index: &HashSet<ReferenceValue>,
) -> Result<(), ValidationError> {
    for reference in references {
        if !known_contracts.contains(reference.value.contract.as_str()) {
            return Err(ValidationError::Contract {
                path: reference.path,
                message: format!("unknown reference contract: {}", reference.value.contract),
            });
        }
        if !index.contains(&reference.value) {
            return Err(ValidationError::UnresolvedReference {
                path: reference.path,
                contract: reference.value.contract,
                id: reference.value.id,
                revision: reference.value.revision,
            });
        }
    }
    Ok(())
}

fn validate_contract(
    contract: &CatalogContract,
    contract_path: &str,
) -> Result<(), ValidationError> {
    validate_entries_path(&contract.entries_path).map_err(|message| ValidationError::Contract {
        path: contract_path.to_string(),
        message,
    })?;

    let mut names = HashSet::with_capacity(contract.required_files.len());
    for required in &contract.required_files {
        if !is_safe_component(&required.name) {
            return Err(ValidationError::Contract {
                path: contract_path.to_string(),
                message: format!(
                    "required file name is not a safe component: {}",
                    required.name
                ),
            });
        }
        if !names.insert(&required.name) {
            return Err(ValidationError::Contract {
                path: contract_path.to_string(),
                message: format!("duplicate required file: {}", required.name),
            });
        }
        let schema_name = required.schema.strip_prefix("luma-forge://schema/");
        if !schema_name.is_some_and(is_safe_component) {
            return Err(ValidationError::Contract {
                path: contract_path.to_string(),
                message: format!("invalid schema reference: {}", required.schema),
            });
        }
    }

    Ok(())
}

fn validate_entries_path(path: &str) -> Result<(), String> {
    let mut components = Path::new(path).components();
    if !matches!(components.next(), Some(Component::Normal(value)) if value == "catalog")
        || !matches!(components.next(), Some(Component::Normal(value)) if value == "entries")
    {
        return Err("entries_path must be relative under catalog/entries".to_string());
    }

    let mut has_child = false;
    for component in components {
        if !matches!(component, Component::Normal(_)) {
            return Err("entries_path must contain only normal components".to_string());
        }
        has_child = true;
    }

    if has_child {
        Ok(())
    } else {
        Err("entries_path must be relative under catalog/entries".to_string())
    }
}

fn is_safe_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(name)) if name == OsStr::new(value))
        && components.next().is_none()
}

async fn read_directories(
    root: &Path,
    directory: &Path,
) -> Result<Vec<(String, PathBuf)>, ValidationError> {
    let relative = relative_to_root(root, directory);
    reject_symlinks(root, directory).await?;
    let mut directories = Vec::new();
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|source| ValidationError::Io {
            path: relative.clone(),
            source,
        })?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|source| ValidationError::Io {
            path: relative.clone(),
            source,
        })?
    {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| ValidationError::Io {
                path: relative_to_root(root, &path),
                source,
            })?;
        if file_type.is_symlink() {
            return Err(symlink_error(root, &path));
        }
        if !file_type.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ValidationError::Contract {
                path: relative_to_root(root, &path),
                message: "entry directory name is not valid UTF-8".to_string(),
            })?;
        directories.push((name, path));
    }

    Ok(directories)
}

async fn read_direct_files(root: &Path, directory: &Path) -> Result<Vec<PathBuf>, ValidationError> {
    let relative = relative_to_root(root, directory);
    reject_symlinks(root, directory).await?;
    let mut files = Vec::new();
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|source| ValidationError::Io {
            path: relative.clone(),
            source,
        })?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|source| ValidationError::Io {
            path: relative.clone(),
            source,
        })?
    {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| ValidationError::Io {
                path: relative_to_root(root, &path),
                source,
            })?;
        if file_type.is_symlink() {
            return Err(symlink_error(root, &path));
        }
        if file_type.is_file() {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

async fn read_required_json(root: &Path, path: &Path) -> Result<Value, ValidationError> {
    let relative = relative_to_root(root, path);
    reject_symlinks(root, path).await?;
    let bytes = match fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ValidationError::Contract {
                path: relative,
                message: "missing required file".to_string(),
            });
        }
        Err(source) => {
            return Err(ValidationError::Io {
                path: relative,
                source,
            });
        }
    };
    serde_json::from_slice(&bytes).map_err(|source| ValidationError::Json {
        path: relative,
        source,
    })
}

async fn read_json_value(root: &Path, path: &Path) -> Result<Value, ValidationError> {
    let relative = relative_to_root(root, path);
    reject_symlinks(root, path).await?;
    let bytes = fs::read(path).await.map_err(|source| ValidationError::Io {
        path: relative.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ValidationError::Json {
        path: relative,
        source,
    })
}

async fn reject_symlinks(root: &Path, path: &Path) -> Result<(), ValidationError> {
    let mut current = root.to_path_buf();
    for component in path.strip_prefix(root).unwrap_or(path).components() {
        current.push(component);
        match fs::symlink_metadata(&current).await {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(symlink_error(root, &current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(ValidationError::Io {
                    path: relative_to_root(root, &current),
                    source,
                });
            }
        }
    }
    Ok(())
}

fn symlink_error(root: &Path, path: &Path) -> ValidationError {
    ValidationError::Contract {
        path: relative_to_root(root, path),
        message: "symbolic links are not allowed".to_string(),
    }
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| {
            relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| "<bundled-root>".to_string())
}

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct AuditFixture {
    root: PathBuf,
}

impl AuditFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "luma-forge-audit-unit-{}-{}",
            std::process::id(),
            NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
        ));
        let fixture = Self { root };

        fixture.write(
            "catalog/schemas/document",
            r#"{
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$id": "luma-forge://schema/document",
                "type": "object"
            }"#,
        );
        fixture.write(
            "catalog/contracts/source_revision",
            r#"{
                "entries_path": "catalog/entries/sources",
                "required_files": [
                    { "name": "document", "schema": "luma-forge://schema/document" }
                ]
            }"#,
        );
        fixture.write(
            "catalog/contracts/target_revision",
            r#"{
                "entries_path": "catalog/entries/targets",
                "required_files": [
                    { "name": "document", "schema": "luma-forge://schema/document" }
                ]
            }"#,
        );
        fixture.write(
            "catalog/entries/sources/source/1/document",
            r#"{
                "reference": {
                    "contract": "catalog/contracts/target_revision",
                    "id": "target",
                    "revision": "1"
                }
            }"#,
        );
        fixture.write("catalog/entries/targets/target/1/document", "{}");

        fixture
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, value: &str) {
        let path = self.path(relative);
        stdfs::create_dir_all(path.parent().unwrap()).unwrap();
        stdfs::write(path, value).unwrap();
    }
}

impl Drop for AuditFixture {
    fn drop(&mut self) {
        let _ = stdfs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn rejects_a_dangling_reference() {
    let fixture = AuditFixture::new();
    let path = fixture.path("catalog/entries/sources/source/1/document");
    let value = stdfs::read_to_string(&path)
        .unwrap()
        .replace(r#""id": "target""#, r#""id": "missing""#);
    stdfs::write(path, value).unwrap();

    assert!(matches!(
        validate(&fixture.root).await,
        Err(ValidationError::UnresolvedReference {
            contract,
            id,
            revision,
            ..
        }) if contract == "catalog/contracts/target_revision"
            && id == "missing"
            && revision == "1"
    ));
}

#[tokio::test]
async fn rejects_a_missing_contract_schema_without_revisions() {
    let fixture = AuditFixture::new();
    stdfs::remove_dir_all(fixture.path("catalog/entries/sources/source")).unwrap();
    let path = fixture.path("catalog/contracts/source_revision");
    let value = stdfs::read_to_string(&path).unwrap().replace(
        "luma-forge://schema/document",
        "luma-forge://schema/missing",
    );
    stdfs::write(path, value).unwrap();

    assert!(matches!(
        validate(&fixture.root).await,
        Err(ValidationError::Schema { path, .. })
            if path == "catalog/contracts/source_revision"
    ));
}

#[cfg(unix)]
#[test]
fn rejects_a_non_utf8_contract_filename() {
    let root = Path::new("bundled-root");
    let path = root
        .join("catalog/contracts")
        .join(std::ffi::OsString::from_vec(vec![0xff]));

    assert!(matches!(
        contract_identity(root, &path),
        Err(ValidationError::Contract { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_a_symlinked_contract() {
    let fixture = AuditFixture::new();
    let path = fixture.path("catalog/contracts/source_revision");
    let target = fixture.path("outside-contract");
    stdfs::rename(&path, &target).unwrap();
    std::os::unix::fs::symlink(target, &path).unwrap();

    assert!(matches!(
        validate(&fixture.root).await,
        Err(ValidationError::Contract { path, .. })
            if path == "catalog/contracts/source_revision"
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_a_symlinked_revision() {
    let fixture = AuditFixture::new();
    let path = fixture.path("catalog/entries/sources/source/1");
    let target = fixture.path("outside-revision");
    stdfs::rename(&path, &target).unwrap();
    std::os::unix::fs::symlink(target, &path).unwrap();

    assert!(matches!(
        validate(&fixture.root).await,
        Err(ValidationError::Contract { path, .. })
            if path == "catalog/entries/sources/source/1"
    ));
}
