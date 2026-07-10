# Test-Only Bundled Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the production `Catalog::validate` API and compile the complete bundled catalog audit only inside an internal `#[cfg(test)] validation.rs` module.

**Architecture:** `catalog.rs` remains the production storage engine and exposes only the minimum `pub(super)` storage primitives needed by its test-only sibling. `validation.rs` owns schema compilation, complete descriptor/reference auditing, private validation errors, fixtures, and CI audit tests. Runtime read integration tests remain external and never access validation.

**Tech Stack:** Rust 2021, Tokio filesystem APIs, serde/serde_json, thiserror, jsonschema 0.46.6 as a dev-dependency.

## Global Constraints

- `Catalog` stores only the bundled root and exposes no validation method or audit lifecycle.
- Declare validation as `#[cfg(test)] mod validation;`; it must not be part of production builds or the public API.
- Keep ordinary `Entry::get` and `Entry::all` behavior unchanged, including contract validation, path safety, symlink rejection, sequential reads, and relative errors.
- Keep runtime read integration tests in `src-tauri/tests/bundled_catalog.rs`; move only full-audit assertions into the internal validation module.
- Keep schema values, compiled validators, descriptors, and reference indexes operation-local to one audit.
- Use a private `ValidationError`; remove audit-only `Schema` and `UnresolvedReference` variants from `BundledCatalogError`.
- Move `jsonschema` from normal dependencies to dev-dependencies. Keep `regress`; Typify-generated runtime DTO validation requires it.
- Do not add caches, concurrency, registries of Rust entry types, compatibility code, new dependencies, or additional production modules.
- Do not edit generated Rust output manually.
- Preserve unrelated SQLite and other user-owned worktree changes. Use scoped staging and a Conventional Commit.

---

### Task 1: Move the Full Audit Behind `cfg(test)`

**Files:**
- Create: `src-tauri/src/infra/bundled/validation.rs`
- Modify: `src-tauri/src/infra/bundled/mod.rs`
- Modify: `src-tauri/src/infra/bundled/catalog.rs`
- Modify: `src-tauri/src/infra/bundled/errors.rs`
- Modify: `src-tauri/tests/bundled_catalog.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify only if Cargo changes it: `src-tauri/Cargo.lock`

**Interfaces:**
- Consumes: production `Catalog::new`, contract parsing, revision discovery, JSON readers, safe-component checks, symlink rejection, and relative-path formatting from `catalog.rs`.
- Produces: private `validation::validate(root: &Path) -> Result<(), ValidationError>` and internal CI audit tests.
- Removes: public `Catalog::validate`, `BundledCatalogError::Schema`, and `BundledCatalogError::UnresolvedReference`.

- [ ] **Step 1: Add the test-only module and a failing packaged-catalog test**

Add to `src-tauri/src/infra/bundled/mod.rs`:

```rust
mod catalog;
pub mod entries;
pub mod errors;
#[allow(clippy::large_enum_variant)]
pub mod generated;
#[cfg(test)]
mod validation;

pub use catalog::Catalog;
pub use errors::BundledCatalogError;
```

Create `src-tauri/src/infra/bundled/validation.rs` with the first observable test but no `validate` implementation yet:

```rust
use std::path::{Path, PathBuf};

fn bundled_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled")
}

#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    validate(&bundled_root()).await.unwrap();
}
```

- [ ] **Step 2: Run the new unit test to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib infra::bundled::validation::packaged_catalog_passes_full_audit
```

Expected: FAIL with `cannot find function validate in this scope`.

- [ ] **Step 3: Reduce `catalog.rs` to production reads plus shared storage primitives**

Remove these audit-only imports, types, implementations, and functions from `catalog.rs`:

```text
SchemaRetriever
Schemas
ReferenceValue
LocatedReference
LocatedContract
AuditDescriptor
Schemas::load
Catalog::validate
Catalog::read_all_contracts
Catalog::audit_descriptors
Catalog::read_validated_references
collect_references
validate_references
jsonschema references
```

Keep the production imports needed by reads:

```rust
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use tokio::fs;
```

Give the sibling validation module access only to the existing storage types and helpers it reuses:

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CatalogContract {
    pub(super) entries_path: String,
    pub(super) required_files: Vec<RequiredFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequiredFile {
    pub(super) name: String,
    pub(super) schema: String,
}

pub(super) struct RevisionDescriptor {
    pub(super) id: String,
    pub(super) revision: String,
    pub(super) path: PathBuf,
}
```

Change only these existing boundaries to `pub(super)`; leave their bodies unchanged:

```rust
impl Catalog {
    pub(super) async fn read_contract(
        &self,
        contract_path: &str,
    ) -> Result<CatalogContract, BundledCatalogError>;

    pub(super) async fn revision_descriptors(
        &self,
        contract: &CatalogContract,
    ) -> Result<Vec<RevisionDescriptor>, BundledCatalogError>;
}

pub(super) fn is_safe_component(value: &str) -> bool;

#[cfg(test)]
pub(super) async fn read_direct_files(
    root: &Path,
    directory: &Path,
) -> Result<Vec<PathBuf>, BundledCatalogError>;

pub(super) async fn read_required_json(
    root: &Path,
    path: &Path,
) -> Result<Value, BundledCatalogError>;

pub(super) async fn read_json_value(
    root: &Path,
    path: &Path,
) -> Result<Value, BundledCatalogError>;

pub(super) fn relative_to_root(root: &Path, path: &Path) -> String;
```

Do not expose `Catalog::root`, `read_directories`, `reject_symlinks`, or `symlink_error`; the shared functions above already preserve the production path policy.

- [ ] **Step 4: Implement the private validation engine in `validation.rs`**

Replace the initial file body with these imports, audit types, and private error boundary:

```rust
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Deserialize;
use serde_json::Value;

use super::{
    catalog::{
        is_safe_component, read_direct_files, read_json_value, read_required_json,
        relative_to_root, CatalogContract, RevisionDescriptor,
    },
    BundledCatalogError, Catalog,
};

#[derive(Debug, thiserror::Error)]
enum ValidationError {
    #[error(transparent)]
    Catalog(#[from] BundledCatalogError),
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
```

Load and compile schemas with the existing single-read/single-compile behavior:

```rust
impl Schemas {
    async fn load(root: &Path) -> Result<Self, ValidationError> {
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
```

Implement the private audit entry point and helpers with these exact signatures and control flow:

```rust
async fn validate(root: &Path) -> Result<(), ValidationError> {
    let catalog = Catalog::new(root);
    let contracts = read_all_contracts(root, &catalog).await?;
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

    let descriptors = audit_descriptors(&catalog, &contracts).await?;
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
        let references = read_validated_references(
            root,
            descriptor,
            &located.contract,
            &schemas,
        )
        .await?;
        validate_references(references, &known_contracts, &index)?;
    }

    Ok(())
}

async fn read_all_contracts(
    root: &Path,
    catalog: &Catalog,
) -> Result<Vec<LocatedContract>, ValidationError> {
    let files = read_direct_files(root, &root.join("catalog/contracts")).await?;
    let mut contracts = Vec::with_capacity(files.len());
    let mut entries_paths = HashSet::with_capacity(files.len());

    for path in files {
        let contract_path = relative_to_root(root, &path);
        let contract = catalog.read_contract(&contract_path).await?;
        if !entries_paths.insert(contract.entries_path.clone()) {
            return Err(BundledCatalogError::Contract {
                path: contract_path,
                message: format!("duplicate entries_path: {}", contract.entries_path),
            }
            .into());
        }
        contracts.push(LocatedContract {
            path: contract_path,
            contract,
        });
    }

    Ok(contracts)
}

async fn audit_descriptors(
    catalog: &Catalog,
    contracts: &[LocatedContract],
) -> Result<Vec<AuditDescriptor>, ValidationError> {
    let mut descriptors = Vec::new();
    for (contract_index, located) in contracts.iter().enumerate() {
        for RevisionDescriptor { id, revision, path } in
            catalog.revision_descriptors(&located.contract).await?
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
```

Add the remaining audit helpers:

```rust
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
            return Err(BundledCatalogError::Contract {
                path: reference.path,
                message: format!("unknown reference contract: {}", reference.value.contract),
            }
            .into());
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
```

- [ ] **Step 5: Move audit tests into `validation.rs` and leave runtime tests external**

Add these audit tests below the validation helpers:

```rust
#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    validate(&bundled_root()).await.unwrap();
}

#[tokio::test]
async fn audit_rejects_a_dangling_reference() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root
        .join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements");
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace("\"id\": \"provisioner\"", "\"id\": \"missing\"");
    fs::write(&path, value).unwrap();

assert!(matches!(
    validate(&temp_root).await,
    Err(ValidationError::UnresolvedReference {
        contract,
        id,
        revision,
        ..
    }) if contract == "catalog/contracts/runtime_contract_revision"
        && id == "missing"
        && revision == "1.0.0"
));

    fs::remove_dir_all(temp_root).unwrap();
}

#[tokio::test]
async fn audit_rejects_a_missing_contract_schema_without_revisions() {
    let temp_root = copy_catalog_fixture();
    fs::remove_dir_all(temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev"))
        .unwrap();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    let value = fs::read_to_string(&path).unwrap().replace(
        "luma-forge://schema/workflow_metadata",
        "luma-forge://schema/missing",
    );
    fs::write(&path, value).unwrap();

assert!(matches!(
    validate(&temp_root).await,
    Err(ValidationError::Schema { path, .. })
        if path == "catalog/contracts/workflow_revision"
));

    fs::remove_dir_all(temp_root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn audit_rejects_a_symlinked_contract() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(
        bundled_root().join("catalog/contracts/workflow_revision"),
        &path,
    )
    .unwrap();

    assert!(matches!(
        validate(&temp_root).await,
        Err(ValidationError::Catalog(BundledCatalogError::Contract { path, .. }))
            if path == "catalog/contracts/workflow_revision"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn audit_rejects_a_symlinked_revision() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0");
    fs::remove_dir_all(&path).unwrap();
    std::os::unix::fs::symlink(
        bundled_root().join(
            "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0",
        ),
        &path,
    )
    .unwrap();

    assert!(matches!(
        validate(&temp_root).await,
        Err(ValidationError::Catalog(BundledCatalogError::Contract { path, .. }))
            if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}
```

Keep the two Unix runtime assertions in the integration test and rename them:

```rust
#[cfg(unix)]
#[tokio::test]
async fn reads_reject_a_symlinked_contract() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../new_bundled/catalog/contracts/workflow_revision"),
        &path,
    )
    .unwrap();

    let read = workflows::Entry::all(&Catalog::new(&temp_root)).await;
    assert!(matches!(
        read,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/contracts/workflow_revision"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn get_rejects_a_symlinked_revision() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0");
    fs::remove_dir_all(&path).unwrap();
    std::os::unix::fs::symlink(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"),
        &path,
    )
    .unwrap();

    let read = workflows::Entry::get(
        &Catalog::new(&temp_root),
        ("comfyui-hidream-o1-dev", "1.0.0"),
    )
    .await;
    assert!(matches!(
        read,
        Err(BundledCatalogError::Contract { path, .. })
            if path == "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0"
    ));

    fs::remove_dir_all(temp_root).unwrap();
}
```

Add the same small fixture helpers to the private validation module; do not add a shared test-support module:

```rust
fn bundled_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled")
}

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn copy_catalog_fixture() -> PathBuf {
    let target = std::env::temp_dir().join(format!(
        "luma-forge-validation-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
    ));
    copy_dir_all(&bundled_root(), &target);
    target
}

fn copy_dir_all(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
```

- [ ] **Step 6: Remove the production audit API and dependency surface**

Reduce `BundledCatalogError` in `src-tauri/src/infra/bundled/errors.rs` to runtime variants only:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundledCatalogError {
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
    #[error("bundled catalog entry error at {path}: {message}")]
    Entry { path: String, message: String },
}
```

Move `jsonschema` in `src-tauri/Cargo.toml` without changing its version or features:

```toml
[dev-dependencies]
jsonschema = { version = "0.46.6", default-features = false }
```

Remove its line from `[dependencies]`. Run `cargo check` before staging; include `src-tauri/Cargo.lock` only if Cargo actually changes it.

- [ ] **Step 7: Run focused tests to verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib infra::bundled::validation
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
```

Expected:

- all internal audit tests pass against real and copied catalogs;
- all runtime integration tests pass without importing validation or calling `Catalog::validate`.

- [ ] **Step 8: Verify the production boundary and complete native checks**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --lib
rg -n 'Catalog::validate|pub async fn validate|BundledCatalogError::(Schema|UnresolvedReference)' src-tauri/src src-tauri/tests
cargo tree --manifest-path src-tauri/Cargo.toml --edges normal | rg 'jsonschema'
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected:

- `cargo check --lib` passes without compiling audit code;
- both `rg` commands print no matches (exit 1 is expected for `rg`);
- all tests and formatting pass;
- strict Clippy may fail only on the pre-existing unused `SqliteInfraError::statement_failed` outside this task.

If and only if that unchanged SQLite warning is the sole strict-Clippy failure, run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings -A dead-code
```

Expected: PASS with every non-dead-code warning denied.

- [ ] **Step 9: Commit the test-only validation refactor**

Review scope first:

```bash
git diff --check
git status --short
```

Stage only the implementation slice:

```bash
git add src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/src/infra/bundled/errors.rs \
  src-tauri/src/infra/bundled/mod.rs \
  src-tauri/src/infra/bundled/validation.rs \
  src-tauri/tests/bundled_catalog.rs \
  src-tauri/Cargo.toml
```

Add `src-tauri/Cargo.lock` only if it changed, then commit:

```bash
git commit -m "refactor(bundled): make catalog validation test-only"
```
