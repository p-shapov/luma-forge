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
        let path = entries_root.join(id).join(revision);
        reject_symlinks(&self.root, &path).await?;
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
    reject_symlinks(root, directory).await?;
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
        let file_type = entry
            .file_type()
            .await
            .map_err(|source| BundledCatalogError::Io {
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
            .map_err(|_| BundledCatalogError::Contract {
                path: relative_to_root(root, &path),
                message: "entry directory name is not valid UTF-8".to_string(),
            })?;
        directories.push((name, path));
    }

    Ok(directories)
}

async fn read_required_json(root: &Path, path: &Path) -> Result<Value, BundledCatalogError> {
    let relative = relative_to_root(root, path);
    reject_symlinks(root, path).await?;
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
    reject_symlinks(root, path).await?;
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

async fn reject_symlinks(root: &Path, path: &Path) -> Result<(), BundledCatalogError> {
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
                return Err(BundledCatalogError::Io {
                    path: relative_to_root(root, &current),
                    source,
                });
            }
        }
    }
    Ok(())
}

fn symlink_error(root: &Path, path: &Path) -> BundledCatalogError {
    BundledCatalogError::Contract {
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

#[cfg(test)]
mod tests {
    use std::{
        fs as stdfs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde::Deserialize;

    use super::*;

    static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestEntry;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestDocument {
        value: String,
    }

    #[derive(Debug, PartialEq)]
    struct TestModel {
        id: String,
        revision: String,
        document: TestDocument,
    }

    impl CatalogEntry for TestEntry {
        type Model = TestModel;

        const CONTRACT: &'static str = "catalog/contracts/test_revision";

        fn decode(
            id: String,
            revision: String,
            mut documents: Documents,
        ) -> Result<TestModel, BundledCatalogError> {
            Ok(TestModel {
                id,
                revision,
                document: documents.take("document")?,
            })
        }
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "luma-forge-catalog-unit-{}-{}",
                std::process::id(),
                NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
            ));
            let fixture = Self { root };
            fixture.write_contract(
                r#"{
                    "entries_path": "catalog/entries/tests",
                    "required_files": [
                        { "name": "document", "schema": "luma-forge://schema/test" }
                    ]
                }"#,
            );
            fixture.write_document("item", "1.0.0", "selected");
            fixture
        }

        fn catalog(&self) -> Catalog {
            Catalog::new(&self.root)
        }

        fn contract_path(&self) -> PathBuf {
            self.root.join("catalog/contracts/test_revision")
        }

        fn revision_path(&self, id: &str, revision: &str) -> PathBuf {
            self.root
                .join("catalog/entries/tests")
                .join(id)
                .join(revision)
        }

        fn write_contract(&self, value: &str) {
            let path = self.contract_path();
            stdfs::create_dir_all(path.parent().unwrap()).unwrap();
            stdfs::write(path, value).unwrap();
        }

        fn write_document(&self, id: &str, revision: &str, value: &str) {
            let path = self.revision_path(id, revision).join("document");
            stdfs::create_dir_all(path.parent().unwrap()).unwrap();
            stdfs::write(path, format!(r#"{{"value":"{value}"}}"#)).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = stdfs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn construction_performs_no_io() {
        let _catalog = Catalog::new("missing-bundled-root");
    }

    #[tokio::test]
    async fn all_and_get_read_owned_models() {
        let fixture = Fixture::new();
        let catalog = fixture.catalog();

        assert_eq!(
            catalog.all::<TestEntry>().await.unwrap(),
            vec![TestModel {
                id: "item".to_string(),
                revision: "1.0.0".to_string(),
                document: TestDocument {
                    value: "selected".to_string(),
                },
            }]
        );
        assert_eq!(
            catalog.get::<TestEntry>(("item", "1.0.0")).await.unwrap(),
            Some(TestModel {
                id: "item".to_string(),
                revision: "1.0.0".to_string(),
                document: TestDocument {
                    value: "selected".to_string(),
                },
            })
        );
        assert!(catalog
            .get::<TestEntry>(("missing", "1.0.0"))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn get_reads_only_the_selected_revision_without_schemas() {
        let fixture = Fixture::new();
        fixture.write_document("selected", "1.0.0", "isolated");
        stdfs::write(fixture.revision_path("item", "1.0.0").join("document"), "{").unwrap();

        let model = fixture
            .catalog()
            .get::<TestEntry>(("selected", "1.0.0"))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(model.document.value, "isolated");
    }

    #[tokio::test]
    async fn get_rejects_a_missing_selected_document() {
        let fixture = Fixture::new();
        stdfs::remove_file(fixture.revision_path("item", "1.0.0").join("document")).unwrap();

        assert!(matches!(
            fixture.catalog().get::<TestEntry>(("item", "1.0.0")).await,
            Err(BundledCatalogError::Contract { .. })
        ));
    }

    #[tokio::test]
    async fn get_rejects_unsafe_keys() {
        let fixture = Fixture::new();
        let catalog = fixture.catalog();

        for key in [("../outside", "1.0.0"), ("item", "../1.0.0")] {
            assert!(matches!(
                catalog.get::<TestEntry>(key).await,
                Err(BundledCatalogError::Contract { path, .. }) if path == "catalog/entries"
            ));
        }
    }

    #[tokio::test]
    async fn reads_reject_retired_contract_fields() {
        let fixture = Fixture::new();
        fixture.write_contract(
            r#"{
                "entity": "test",
                "entries_path": "catalog/entries/tests",
                "required_files": [
                    { "name": "document", "schema": "luma-forge://schema/test" }
                ]
            }"#,
        );

        assert!(matches!(
            fixture.catalog().all::<TestEntry>().await,
            Err(BundledCatalogError::Contract { .. })
        ));
    }

    #[tokio::test]
    async fn reads_reject_entries_path_traversal() {
        let fixture = Fixture::new();
        fixture.write_contract(
            r#"{
                "entries_path": "catalog/entries/../outside",
                "required_files": [
                    { "name": "document", "schema": "luma-forge://schema/test" }
                ]
            }"#,
        );

        assert!(matches!(
            fixture.catalog().all::<TestEntry>().await,
            Err(BundledCatalogError::Contract { .. })
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reads_reject_a_symlinked_contract() {
        let fixture = Fixture::new();
        let path = fixture.contract_path();
        let target = fixture.root.join("outside-contract");
        stdfs::rename(&path, &target).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();

        assert!(matches!(
            fixture.catalog().all::<TestEntry>().await,
            Err(BundledCatalogError::Contract { path, .. })
                if path == "catalog/contracts/test_revision"
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn get_rejects_a_symlinked_revision() {
        let fixture = Fixture::new();
        let path = fixture.revision_path("item", "1.0.0");
        let target = fixture.root.join("outside-revision");
        stdfs::rename(&path, &target).unwrap();
        std::os::unix::fs::symlink(target, &path).unwrap();

        assert!(matches!(
            fixture.catalog().get::<TestEntry>(("item", "1.0.0")).await,
            Err(BundledCatalogError::Contract { path, .. })
                if path == "catalog/entries/tests/item/1.0.0"
        ));
    }
}
