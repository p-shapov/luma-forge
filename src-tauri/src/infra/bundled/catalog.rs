use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use tokio::fs;

use super::{
    entries::{CatalogEntry, Documents},
    errors::BundledCatalogError,
};

#[derive(Debug)]
pub struct Catalog {
    root: PathBuf,
}

#[derive(Deserialize)]
struct CatalogContract {
    entries_path: String,
    required_files: Vec<RequiredFile>,
}

#[derive(Deserialize)]
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
    async fn load(root: &Path) -> Result<Self, BundledCatalogError> {
        let directory = root.join("catalog/schemas");
        let files = read_direct_files(root, &directory).await?;
        let mut values = HashMap::with_capacity(files.len());
        let mut schemas = Vec::with_capacity(files.len());

        for path in files {
            let relative = relative_to_root(root, &path);
            let name = path
                .file_name()
                .and_then(OsStr::to_str)
                .filter(|name| is_safe_component(name))
                .ok_or_else(|| BundledCatalogError::Schema {
                    path: relative.clone(),
                    message: "schema filename must be a safe UTF-8 component".to_string(),
                })?;
            let value = read_json_value(root, &path).await?;
            let expected_id = format!("luma-forge://schema/{name}");
            let id = value
                .get("$id")
                .and_then(Value::as_str)
                .ok_or_else(|| BundledCatalogError::Schema {
                    path: relative.clone(),
                    message: "schema must have a string $id".to_string(),
                })?
                .to_string();
            if id != expected_id {
                return Err(BundledCatalogError::Schema {
                    path: relative,
                    message: format!("schema $id must be {expected_id}"),
                });
            }
            if values.insert(id.clone(), value).is_some() {
                return Err(BundledCatalogError::Schema {
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
                .map_err(|error| BundledCatalogError::Schema {
                    path,
                    message: error.to_string(),
                })?;
            validators.insert(id, validator);
        }

        Ok(Self { values, validators })
    }
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub async fn validate(&self) -> Result<(), BundledCatalogError> {
        let contracts = self.read_all_contracts().await?;
        let schemas = Schemas::load(&self.root).await?;
        let descriptors = self.audit_descriptors(&contracts).await?;
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
            let references = self
                .read_validated_references(descriptor, &located.contract, &schemas)
                .await?;
            validate_references(references, &known_contracts, &index)?;
        }

        Ok(())
    }

    pub(super) async fn all<E: CatalogEntry>(&self) -> Result<Vec<E::Model>, BundledCatalogError> {
        let contract = self.read_contract(E::CONTRACT).await?;
        let descriptors = self.revision_descriptors(&contract).await?;
        let mut models = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            models.push(self.read_model::<E>(&contract, descriptor).await?);
        }
        Ok(models)
    }

    pub(super) async fn get<E: CatalogEntry>(
        &self,
        (id, revision): (&str, &str),
    ) -> Result<Option<E::Model>, BundledCatalogError> {
        require_safe_component(id, "id")?;
        require_safe_component(revision, "revision")?;
        let contract = self.read_contract(E::CONTRACT).await?;
        let Some(descriptor) = self.revision_descriptor(&contract, id, revision).await? else {
            return Ok(None);
        };
        self.read_model::<E>(&contract, descriptor).await.map(Some)
    }

    async fn read_contract(
        &self,
        contract_path: &str,
    ) -> Result<CatalogContract, BundledCatalogError> {
        validate_contract_path(contract_path)?;
        let path = self.root.join(contract_path);
        let relative = relative_to_root(&self.root, &path);
        let value = read_json_value(&self.root, &path).await?;
        let contract = serde_json::from_value::<CatalogContract>(value).map_err(|error| {
            BundledCatalogError::Contract {
                path: relative.clone(),
                message: error.to_string(),
            }
        })?;
        validate_contract(&contract, &relative)?;
        Ok(contract)
    }

    async fn read_all_contracts(&self) -> Result<Vec<LocatedContract>, BundledCatalogError> {
        let directory = self.root.join("catalog/contracts");
        let files = read_direct_files(&self.root, &directory).await?;
        let mut contracts = Vec::with_capacity(files.len());
        let mut entries_paths = HashSet::with_capacity(files.len());

        for path in files {
            let contract_path = relative_to_root(&self.root, &path);
            let contract = self.read_contract(&contract_path).await?;
            if !entries_paths.insert(contract.entries_path.clone()) {
                return Err(BundledCatalogError::Contract {
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

    async fn audit_descriptors(
        &self,
        contracts: &[LocatedContract],
    ) -> Result<Vec<AuditDescriptor>, BundledCatalogError> {
        let mut audit_descriptors = Vec::new();
        for (contract_index, located) in contracts.iter().enumerate() {
            for descriptor in self.revision_descriptors(&located.contract).await? {
                audit_descriptors.push(AuditDescriptor {
                    contract_index,
                    id: descriptor.id,
                    revision: descriptor.revision,
                    path: descriptor.path,
                });
            }
        }
        Ok(audit_descriptors)
    }

    async fn read_validated_references(
        &self,
        descriptor: &AuditDescriptor,
        contract: &CatalogContract,
        schemas: &Schemas,
    ) -> Result<Vec<LocatedReference>, BundledCatalogError> {
        let mut references = Vec::new();
        for required in &contract.required_files {
            let path = descriptor.path.join(&required.name);
            let relative = relative_to_root(&self.root, &path);
            let value = read_required_json(&self.root, &path).await?;
            if !schemas.values.contains_key(&required.schema) {
                return Err(BundledCatalogError::Schema {
                    path: relative,
                    message: format!("schema not found: {}", required.schema),
                });
            }
            let validator = schemas.validators.get(&required.schema).ok_or_else(|| {
                BundledCatalogError::Schema {
                    path: relative.clone(),
                    message: format!("validator not found: {}", required.schema),
                }
            })?;
            validator
                .validate(&value)
                .map_err(|error| BundledCatalogError::Schema {
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

    async fn revision_descriptors(
        &self,
        contract: &CatalogContract,
    ) -> Result<Vec<RevisionDescriptor>, BundledCatalogError> {
        let entries_root = self.root.join(&contract.entries_path);
        let mut descriptors = Vec::new();

        for (id, id_path) in read_directories(&self.root, &entries_root).await? {
            for (revision, path) in read_directories(&self.root, &id_path).await? {
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

    async fn revision_descriptor(
        &self,
        contract: &CatalogContract,
        id: &str,
        revision: &str,
    ) -> Result<Option<RevisionDescriptor>, BundledCatalogError> {
        let entries_root = self.root.join(&contract.entries_path);
        let entries_metadata =
            fs::metadata(&entries_root)
                .await
                .map_err(|source| BundledCatalogError::Io {
                    path: relative_to_root(&self.root, &entries_root),
                    source,
                })?;
        if !entries_metadata.is_dir() {
            return Err(BundledCatalogError::Contract {
                path: relative_to_root(&self.root, &entries_root),
                message: "entries path is not a directory".to_string(),
            });
        }

        let path = entries_root.join(id).join(revision);
        match fs::metadata(&path).await {
            Ok(metadata) if metadata.is_dir() => Ok(Some(RevisionDescriptor {
                id: id.to_string(),
                revision: revision.to_string(),
                path,
            })),
            Ok(_) => Err(BundledCatalogError::Contract {
                path: relative_to_root(&self.root, &path),
                message: "revision path is not a directory".to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(BundledCatalogError::Io {
                path: relative_to_root(&self.root, &path),
                source,
            }),
        }
    }

    async fn read_model<E: CatalogEntry>(
        &self,
        contract: &CatalogContract,
        descriptor: RevisionDescriptor,
    ) -> Result<E::Model, BundledCatalogError> {
        let relative = relative_to_root(&self.root, &descriptor.path);
        let mut values = HashMap::with_capacity(contract.required_files.len());

        for required in &contract.required_files {
            let path = descriptor.path.join(&required.name);
            let value = read_required_json(&self.root, &path).await?;
            values.insert(required.name.clone(), value);
        }

        E::decode(
            descriptor.id,
            descriptor.revision,
            Documents::new(relative, values),
        )
    }
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
) -> Result<(), BundledCatalogError> {
    for reference in references {
        if !known_contracts.contains(reference.value.contract.as_str()) {
            return Err(BundledCatalogError::Contract {
                path: reference.path,
                message: format!("unknown reference contract: {}", reference.value.contract),
            });
        }
        if !index.contains(&reference.value) {
            return Err(BundledCatalogError::UnresolvedReference {
                path: reference.path,
                contract: reference.value.contract,
                id: reference.value.id,
                revision: reference.value.revision,
            });
        }
    }
    Ok(())
}

fn validate_contract_path(path: &str) -> Result<(), BundledCatalogError> {
    let mut components = Path::new(path).components();
    let valid = matches!(components.next(), Some(Component::Normal(value)) if value == "catalog")
        && matches!(components.next(), Some(Component::Normal(value)) if value == "contracts")
        && matches!(components.next(), Some(Component::Normal(value)) if value.to_str().is_some_and(is_safe_component))
        && components.next().is_none();

    if valid {
        Ok(())
    } else {
        Err(BundledCatalogError::Contract {
            path: "catalog/contracts".to_string(),
            message: "contract path must be catalog/contracts/<safe-name>".to_string(),
        })
    }
}

fn validate_contract(
    contract: &CatalogContract,
    contract_path: &str,
) -> Result<(), BundledCatalogError> {
    validate_entries_path(&contract.entries_path).map_err(|message| {
        BundledCatalogError::Contract {
            path: contract_path.to_string(),
            message,
        }
    })?;

    let mut names = HashSet::with_capacity(contract.required_files.len());
    for required in &contract.required_files {
        if !is_safe_component(&required.name) {
            return Err(BundledCatalogError::Contract {
                path: contract_path.to_string(),
                message: format!(
                    "required file name is not a safe component: {}",
                    required.name
                ),
            });
        }
        if !names.insert(&required.name) {
            return Err(BundledCatalogError::Contract {
                path: contract_path.to_string(),
                message: format!("duplicate required file: {}", required.name),
            });
        }
        let schema_name = required.schema.strip_prefix("luma-forge://schema/");
        if !schema_name.is_some_and(is_safe_component) {
            return Err(BundledCatalogError::Contract {
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

fn require_safe_component(value: &str, label: &str) -> Result<(), BundledCatalogError> {
    if is_safe_component(value) {
        Ok(())
    } else {
        Err(BundledCatalogError::Contract {
            path: "catalog/entries".to_string(),
            message: format!("{label} must be a safe path component"),
        })
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
) -> Result<Vec<(String, PathBuf)>, BundledCatalogError> {
    let relative = relative_to_root(root, directory);
    let mut directories = Vec::new();
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|source| BundledCatalogError::Io {
            path: relative.clone(),
            source,
        })?;

    while let Some(entry) =
        entries
            .next_entry()
            .await
            .map_err(|source| BundledCatalogError::Io {
                path: relative.clone(),
                source,
            })?
    {
        let path = entry.path();
        if !entry
            .file_type()
            .await
            .map_err(|source| BundledCatalogError::Io {
                path: relative_to_root(root, &path),
                source,
            })?
            .is_dir()
        {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| BundledCatalogError::Contract {
                path: relative_to_root(root, &path),
                message: "entry directory name is not valid UTF-8".to_string(),
            })?;
        directories.push((name, path));
    }

    Ok(directories)
}

async fn read_direct_files(
    root: &Path,
    directory: &Path,
) -> Result<Vec<PathBuf>, BundledCatalogError> {
    let relative = relative_to_root(root, directory);
    let mut files = Vec::new();
    let mut entries = fs::read_dir(directory)
        .await
        .map_err(|source| BundledCatalogError::Io {
            path: relative.clone(),
            source,
        })?;

    while let Some(entry) =
        entries
            .next_entry()
            .await
            .map_err(|source| BundledCatalogError::Io {
                path: relative.clone(),
                source,
            })?
    {
        let path = entry.path();
        if entry
            .file_type()
            .await
            .map_err(|source| BundledCatalogError::Io {
                path: relative_to_root(root, &path),
                source,
            })?
            .is_file()
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

async fn read_required_json(root: &Path, path: &Path) -> Result<Value, BundledCatalogError> {
    let relative = relative_to_root(root, path);
    let bytes = match fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BundledCatalogError::Contract {
                path: relative,
                message: "missing required file".to_string(),
            });
        }
        Err(source) => {
            return Err(BundledCatalogError::Io {
                path: relative,
                source,
            });
        }
    };
    serde_json::from_slice(&bytes).map_err(|source| BundledCatalogError::Json {
        path: relative,
        source,
    })
}

async fn read_json_value(root: &Path, path: &Path) -> Result<Value, BundledCatalogError> {
    let relative = relative_to_root(root, path);
    let bytes = fs::read(path)
        .await
        .map_err(|source| BundledCatalogError::Io {
            path: relative.clone(),
            source,
        })?;
    serde_json::from_slice(&bytes).map_err(|source| BundledCatalogError::Json {
        path: relative,
        source,
    })
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
