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

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
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
            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(BundledCatalogError::Contract {
                        path: relative_to_root(&self.root, &path),
                        message: "missing required file".to_string(),
                    });
                }
                Err(source) => {
                    return Err(BundledCatalogError::Io {
                        path: relative_to_root(&self.root, &path),
                        source,
                    });
                }
            };
            let value = serde_json::from_slice::<Value>(&bytes).map_err(|source| {
                BundledCatalogError::Json {
                    path: relative_to_root(&self.root, &path),
                    source,
                }
            })?;
            values.insert(required.name.clone(), value);
        }

        E::decode(
            descriptor.id,
            descriptor.revision,
            Documents::new(relative, values),
        )
    }
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
