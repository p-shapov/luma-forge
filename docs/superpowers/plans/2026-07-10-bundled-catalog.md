# Bundled Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current bundled aggregate/query builder with a contract-addressed `Entry`/`Model` storage API, lazy typed reads, and a separate CI-only full catalog audit.

**Architecture:** `Catalog` stores only the bundled root. Concrete zero-sized `Entry` types point at one contract and expose direct `all`/`get` methods returning owned `Model` values; ordinary reads parse only the selected contract and documents. `Catalog::validate` separately loads every contract/schema/document, reuses compiled validators within the audit, builds a generic `(contract, id, revision)` index, and verifies references.

**Tech Stack:** Rust 2021, Tokio filesystem APIs, serde/serde_json, thiserror, jsonschema 0.46.x, typify, schemars, syn, prettyplease.

## Global Constraints

- Implement the approved design in `docs/superpowers/specs/2026-07-10-bundled-catalog-design.md` directly; do not preserve the current API.
- `Catalog::new` stores only `PathBuf`, performs no I/O, and cannot fail.
- Ordinary `Entry::get` and `Entry::all` reads do not load schemas, validate references, hydrate relationships, build a global index, or cache state.
- `Catalog::validate` is the only JSON Schema and catalog-reference validation path and is exercised against `new_bundled` by CI tests.
- All JSON objects under `new_bundled/catalog` have no `.json` extension.
- Contracts contain only `entries_path` and `required_files`; they contain no `entity`, regex, path template, or named capture configuration.
- Catalog references contain exactly `contract`, `id`, and `revision`.
- The physical revision layout is always `<entries_path>/<id>/<revision>`.
- `id` and `revision` are opaque UTF-8 safe single path components; domain and semver validation are out of scope.
- Extra files and directories are ignored.
- Reads and audit are sequential; do not add concurrency, persistent caches, registries of Rust entry types, or new dependencies.
- Do not edit generated Rust output manually; update schemas/codegen and let `build.rs` regenerate it.
- Do not add compatibility aliases, fallback paths, migration code, or tests for removed behavior.
- Preserve unrelated SQLite and other user-owned worktree changes. Use scoped staging/commits only.

---

## File Structure

- Modify `src-tauri/src/infra/bundled/codegen.rs`: discover every direct schema file without checking extensions.
- Modify `src-tauri/src/infra/bundled/catalog.rs`: root-only engine, selected-entry reads, contract/schema helpers, and full audit.
- Modify `src-tauri/src/infra/bundled/errors.rs`: path-aware errors from the approved spec.
- Modify `src-tauri/src/infra/bundled/entries/mod.rs`: `CatalogEntry`, `Documents`, and removal of the builder.
- Modify the four files under `src-tauri/src/infra/bundled/entries/`: zero-sized `Entry`, owned `Model`, direct `all`/`get`, explicit decode.
- Modify `src-tauri/Cargo.toml`: remove `regress` and `walkdir`; keep existing Tokio `fs` and `jsonschema` support.
- Create `src-tauri/tests/bundled_catalog.rs`: observable read and audit tests.
- Rename every direct file under `new_bundled/catalog/schemas`, `contracts`, and revision directories to remove `.json`.
- Modify the four contract documents: replace `entity`/`path_pattern` with `entries_path`.
- Modify `new_bundled/catalog/schemas/reference`: replace `entity` with `contract`.
- Modify the three workflow documents containing references: replace entity identifiers with contract paths.

---

### Task 1: Make Extensionless Schemas the Codegen Source

**Files:**
- Modify: `src-tauri/src/infra/bundled/codegen.rs`
- Rename: `new_bundled/catalog/schemas/execution_schema.json` → `new_bundled/catalog/schemas/execution_schema`
- Rename: `new_bundled/catalog/schemas/reference.json` → `new_bundled/catalog/schemas/reference`
- Rename: `new_bundled/catalog/schemas/runtime_contract.json` → `new_bundled/catalog/schemas/runtime_contract`
- Rename: `new_bundled/catalog/schemas/runtime_preset.json` → `new_bundled/catalog/schemas/runtime_preset`
- Rename: `new_bundled/catalog/schemas/workflow_contract_requirements.json` → `new_bundled/catalog/schemas/workflow_contract_requirements`
- Rename: `new_bundled/catalog/schemas/workflow_execution_contract.json` → `new_bundled/catalog/schemas/workflow_execution_contract`
- Rename: `new_bundled/catalog/schemas/workflow_graph.json` → `new_bundled/catalog/schemas/workflow_graph`
- Rename: `new_bundled/catalog/schemas/workflow_metadata.json` → `new_bundled/catalog/schemas/workflow_metadata`
- Rename: `new_bundled/catalog/schemas/workflow_model_assets.json` → `new_bundled/catalog/schemas/workflow_model_assets`

**Interfaces:**
- Consumes: direct JSON files under `new_bundled/catalog/schemas`, regardless of extension.
- Produces: the existing `OUT_DIR/bundled_generated.rs` with the same generated type names.

- [ ] **Step 1: Rename schema files before changing codegen**

Perform the nine exact renames listed above without changing file content.

- [ ] **Step 2: Run the build to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: FAIL because `codegen.rs` filters on `.json`, generates no bundled types, and compilation cannot resolve types such as `generated::WorkflowMetadata`.

- [ ] **Step 3: Remove the extension filter from codegen**

Replace the schema discovery loop with direct-file discovery:

```rust
let mut schemas = Vec::new();
for entry in fs::read_dir(&schema_dir)? {
    let path = entry?.path();
    if path.is_file() {
        schemas.push(path);
    }
}
schemas.sort();
```

Keep the existing per-file `cargo:rerun-if-changed`, reference `$ref` rewrite, typify setup, and generated output path unchanged.

- [ ] **Step 4: Run the build to verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: PASS compilation. Existing bundled generated type names remain available.

- [ ] **Step 5: Commit only schema/codegen changes**

```bash
git add -f new_bundled/catalog/schemas src-tauri/src/infra/bundled/codegen.rs
git commit --only -m "refactor(bundled): use extensionless schemas" -- \
  new_bundled/catalog/schemas \
  src-tauri/src/infra/bundled/codegen.rs
```

---

### Task 2: Replace the Aggregate and Builder with Typed Lazy Reads

**Files:**
- Modify: `src-tauri/src/infra/bundled/catalog.rs`
- Modify: `src-tauri/src/infra/bundled/errors.rs`
- Modify: `src-tauri/src/infra/bundled/entries/mod.rs`
- Modify: `src-tauri/src/infra/bundled/entries/workflows.rs`
- Modify: `src-tauri/src/infra/bundled/entries/runtime_contracts.rs`
- Modify: `src-tauri/src/infra/bundled/entries/runtime_presets.rs`
- Modify: `src-tauri/src/infra/bundled/entries/execution_schemas.rs`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/tests/bundled_catalog.rs`
- Rename: `new_bundled/catalog/contracts/execution_schema_revision.json` → `new_bundled/catalog/contracts/execution_schema_revision`
- Rename: `new_bundled/catalog/contracts/runtime_contract_revision.json` → `new_bundled/catalog/contracts/runtime_contract_revision`
- Rename: `new_bundled/catalog/contracts/runtime_preset_revision.json` → `new_bundled/catalog/contracts/runtime_preset_revision`
- Rename: `new_bundled/catalog/contracts/workflow_revision.json` → `new_bundled/catalog/contracts/workflow_revision`
- Rename: `new_bundled/catalog/entries/execution_schemas/text-to-image/1.0.0/execution_schema.json` → `.../execution_schema`
- Rename: `new_bundled/catalog/entries/runtime_contracts/provisioner/1.0.0/runtime_contract.json` → `.../runtime_contract`
- Rename: `new_bundled/catalog/entries/runtime_contracts/runpod-endpoint-comfyui-hidream-o1-dev/1.0.0/runtime_contract.json` → `.../runtime_contract`
- Rename: `new_bundled/catalog/entries/runtime_presets/comfyui-py312-cu126-torch291/1.0.0/runtime_preset.json` → `.../runtime_preset`
- Rename: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json` → `.../metadata`
- Rename: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/model_assets.json` → `.../model_assets`
- Rename: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json` → `.../contract_requirements`
- Rename: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract.json` → `.../execution_contract`
- Rename: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/workflow.json` → `.../workflow`

**Interfaces:**
- Consumes: extensionless contracts with `{ entries_path, required_files }` and extensionless revision documents.
- Produces: `Catalog::new`, internal generic `Catalog::all::<E>`/`Catalog::get::<E>`, `CatalogEntry`, `Documents`, and public `Entry::all`/`Entry::get` for four entry types.
- Leaves for Task 3: `Catalog::validate` and reference field migration.

- [ ] **Step 1: Write failing public read tests**

Create `src-tauri/tests/bundled_catalog.rs` with a compact API test using the desired interface:

```rust
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use luma_forge_lib::infra::bundled::{
    entries::{execution_schemas, runtime_contracts, runtime_presets, workflows},
    BundledCatalogError, Catalog,
};

fn catalog() -> Catalog {
    Catalog::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
}

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn catalog_construction_performs_no_io() {
    let _catalog = Catalog::new("missing-bundled-root");
}

#[tokio::test]
async fn entry_mappings_read_owned_models() {
    let catalog = catalog();

    assert_eq!(workflows::Entry::all(&catalog).await.unwrap().len(), 1);
    assert_eq!(runtime_contracts::Entry::all(&catalog).await.unwrap().len(), 2);
    assert_eq!(runtime_presets::Entry::all(&catalog).await.unwrap().len(), 1);
    assert_eq!(execution_schemas::Entry::all(&catalog).await.unwrap().len(), 1);

    let workflow = workflows::Entry::get(
        &catalog,
        ("comfyui-hidream-o1-dev", "1.0.0"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
    assert_eq!(workflow.revision, "1.0.0");
    assert!(workflows::Entry::get(&catalog, ("missing", "1.0.0"))
        .await
        .unwrap()
        .is_none());
}
```

Add this second test and its fixture helpers. It copies the workflow revision to `selected/1.0.0`, breaks the original sibling, and removes schemas:

```rust
#[tokio::test]
async fn get_reads_only_the_selected_revision_without_schemas() {
    let temp_root = copy_catalog_fixture();
    let workflows_root = temp_root.join("catalog/entries/workflows");
    copy_dir_all(
        &workflows_root.join("comfyui-hidream-o1-dev/1.0.0"),
        &workflows_root.join("selected/1.0.0"),
    );
    fs::write(
        workflows_root.join("comfyui-hidream-o1-dev/1.0.0/metadata"),
        "{",
    )
    .unwrap();
    fs::remove_dir_all(temp_root.join("catalog/schemas")).unwrap();

    let model = workflows::Entry::get(
        &Catalog::new(&temp_root),
        ("selected", "1.0.0"),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(model.id, "selected");
    fs::remove_dir_all(temp_root).unwrap();
}

fn copy_catalog_fixture() -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled");
    let target = std::env::temp_dir().join(format!(
        "luma-forge-bundled-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
    ));
    copy_dir_all(&source, &target);
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

Add one selected-document failure test without duplicating cases for every entry type:

```rust
#[tokio::test]
async fn get_rejects_a_missing_selected_document() {
    let temp_root = copy_catalog_fixture();
    fs::remove_file(
        temp_root.join(
            "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata",
        ),
    )
    .unwrap();

    assert!(matches!(
        workflows::Entry::get(
            &Catalog::new(&temp_root),
            ("comfyui-hidream-o1-dev", "1.0.0"),
        )
        .await,
        Err(BundledCatalogError::Contract { .. })
    ));

    fs::remove_dir_all(temp_root).unwrap();
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
```

Expected: FAIL to compile because the current entry types expose `find`/`find_by_id` and contain model fields directly; `Entry::all`, `Entry::get`, and `Model` do not exist.

- [ ] **Step 3: Migrate contracts and revision filenames**

Perform every contract and entry-document rename listed in this task.

Replace the four contract bodies with the following shape and exact mappings:

```json
{
  "entries_path": "catalog/entries/workflows",
  "required_files": [
    { "name": "metadata", "schema": "luma-forge://schema/workflow_metadata" },
    { "name": "model_assets", "schema": "luma-forge://schema/workflow_model_assets" },
    { "name": "contract_requirements", "schema": "luma-forge://schema/workflow_contract_requirements" },
    { "name": "execution_contract", "schema": "luma-forge://schema/workflow_execution_contract" },
    { "name": "workflow", "schema": "luma-forge://schema/workflow_graph" }
  ]
}
```

Use these exact one-file mappings for the other contracts:

| Contract | `entries_path` | Required file | Schema |
|---|---|---|---|
| `execution_schema_revision` | `catalog/entries/execution_schemas` | `execution_schema` | `luma-forge://schema/execution_schema` |
| `runtime_contract_revision` | `catalog/entries/runtime_contracts` | `runtime_contract` | `luma-forge://schema/runtime_contract` |
| `runtime_preset_revision` | `catalog/entries/runtime_presets` | `runtime_preset` | `luma-forge://schema/runtime_preset` |

- [ ] **Step 4: Replace the error type**

Implement the approved path-aware variants in `errors.rs`:

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
    #[error("bundled catalog schema error at {path}: {message}")]
    Schema { path: String, message: String },
    #[error("bundled catalog entry error at {path}: {message}")]
    Entry { path: String, message: String },
    #[error("bundled catalog unresolved reference at {path}: {contract}/{id}/{revision}")]
    UnresolvedReference {
        path: String,
        contract: String,
        id: String,
        revision: String,
    },
}
```

Do not retain `Clone`, `PartialEq`, old variant aliases, or string-only I/O/JSON sources.

- [ ] **Step 5: Replace the shared entry API**

In `entries/mod.rs`, delete `Select`, `PhantomData`, `find`, `find_by_id`, `one`, and `ENTITY`. Add:

```rust
pub(super) trait CatalogEntry: Sized {
    type Model;

    const CONTRACT: &'static str;

    fn decode(
        id: String,
        revision: String,
        documents: Documents,
    ) -> Result<Self::Model, BundledCatalogError>;
}

pub(super) struct Documents {
    relative: String,
    values: HashMap<String, Value>,
}

impl Documents {
    pub(super) fn new(relative: String, values: HashMap<String, Value>) -> Self {
        Self { relative, values }
    }

    pub(super) fn take<T: serde::de::DeserializeOwned>(
        &mut self,
        name: &str,
    ) -> Result<T, BundledCatalogError> {
        let path = format!("{}/{name}", self.relative);
        let value = self.values.remove(name).ok_or_else(|| {
            BundledCatalogError::Entry {
                path: path.clone(),
                message: "entry mapping requested an undeclared document".to_string(),
            }
        })?;

        serde_json::from_value(value).map_err(|error| BundledCatalogError::Entry {
            path,
            message: error.to_string(),
        })
    }
}
```

Keep only the four public entry modules plus these shared internal types.

- [ ] **Step 6: Implement the root-only selected-read engine**

Replace the aggregate/query implementation in `catalog.rs` with:

```rust
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
}
```

Use this control flow for the two generic reads:

```rust
impl Catalog {
    pub(super) async fn all<E: CatalogEntry>(
        &self,
    ) -> Result<Vec<E::Model>, BundledCatalogError> {
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
}
```

Implement the method bodies with these exact rules:

- load only `E::CONTRACT` and reject a contract path that is not exactly `catalog/contracts/<safe-name>`;
- validate `entries_path` as a relative path under `catalog/entries` with normal components only;
- validate required-file names as unique safe single components;
- require each `schema` value to have the canonical `luma-forge://schema/<safe-name>` shape, but do not read the schema file during ordinary reads;
- `get` rejects unsafe `id`/`revision`, verifies the entries root, addresses the revision directly, and returns `None` only when that revision directory is absent;
- `all` uses Tokio `read_dir` for the ID level and each revision level, considers directories only, converts names to UTF-8 strings, then sorts descriptors by `(id, revision)`;
- both methods call one `read_model::<E>` helper that reads every required file exactly once, maps `NotFound` to `Contract { message: "missing required file" }`, parses JSON to `Value`, creates `Documents`, and invokes `E::decode`;
- all reported paths use a shared `relative_to_root` helper and never expose the injected absolute root;
- extra files and directories are ignored.

Do not add `Query`, a global descriptor index, schema loading, reference scanning, caching, or concrete entry imports.

- [ ] **Step 7: Convert all four entry modules**

Each module gets a zero-sized `Entry`, an owned `Model`, two direct wrappers, and its exact mapping:

```rust
pub struct Entry;

impl Entry {
    pub async fn all(catalog: &Catalog) -> Result<Vec<Model>, BundledCatalogError> {
        catalog.all::<Self>().await
    }

    pub async fn get(
        catalog: &Catalog,
        key: (&str, &str),
    ) -> Result<Option<Model>, BundledCatalogError> {
        catalog.get::<Self>(key).await
    }
}
```

Use the following exact model/mapping definitions:

```rust
// workflows.rs
#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub metadata: generated::WorkflowMetadata,
    pub model_assets: generated::WorkflowModelAssets,
    pub contract_requirements: generated::WorkflowContractRequirements,
    pub execution_contract: generated::WorkflowExecutionContract,
    pub workflow_graph: generated::WorkflowGraph,
}

impl CatalogEntry for Entry {
    type Model = Model;
    const CONTRACT: &'static str = "catalog/contracts/workflow_revision";

    fn decode(id: String, revision: String, mut documents: Documents) -> Result<Model, BundledCatalogError> {
        Ok(Model {
            id,
            revision,
            metadata: documents.take("metadata")?,
            model_assets: documents.take("model_assets")?,
            contract_requirements: documents.take("contract_requirements")?,
            execution_contract: documents.take("execution_contract")?,
            workflow_graph: documents.take("workflow")?,
        })
    }
}
```

```rust
// runtime_contracts.rs
#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub runtime_contract: generated::RuntimeContract,
}

impl CatalogEntry for Entry {
    type Model = Model;
    const CONTRACT: &'static str = "catalog/contracts/runtime_contract_revision";

    fn decode(id: String, revision: String, mut documents: Documents) -> Result<Model, BundledCatalogError> {
        Ok(Model { id, revision, runtime_contract: documents.take("runtime_contract")? })
    }
}
```

```rust
// runtime_presets.rs
#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub runtime_preset: generated::RuntimePreset,
}

impl CatalogEntry for Entry {
    type Model = Model;
    const CONTRACT: &'static str = "catalog/contracts/runtime_preset_revision";

    fn decode(id: String, revision: String, mut documents: Documents) -> Result<Model, BundledCatalogError> {
        Ok(Model { id, revision, runtime_preset: documents.take("runtime_preset")? })
    }
}
```

```rust
// execution_schemas.rs
#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub execution_schema: generated::ExecutionSchema,
}

impl CatalogEntry for Entry {
    type Model = Model;
    const CONTRACT: &'static str = "catalog/contracts/execution_schema_revision";

    fn decode(id: String, revision: String, mut documents: Documents) -> Result<Model, BundledCatalogError> {
        Ok(Model { id, revision, execution_schema: documents.take("execution_schema")? })
    }
}
```

- [ ] **Step 8: Remove obsolete dependencies and old unit tests**

Remove `regress` and `walkdir` from `src-tauri/Cargo.toml`. Delete the old `Catalog::init`/query-builder tests from `catalog.rs` and `entries/mod.rs`; their removed vocabulary and behavior must not be preserved. Do not remove `jsonschema`, because Task 3 uses it for `Catalog::validate`.

- [ ] **Step 9: Run read tests and the full suite**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: both commands PASS. The new integration tests exercise the four models, `get(None)`, selected-revision isolation, and runtime independence from schemas.

- [ ] **Step 10: Commit only the lazy-read slice**

```bash
git add -f new_bundled/catalog/contracts new_bundled/catalog/entries \
  src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/src/infra/bundled/errors.rs \
  src-tauri/src/infra/bundled/entries \
  src-tauri/tests/bundled_catalog.rs
git commit --only -m "refactor(bundled): add typed lazy entry reads" -- \
  new_bundled/catalog/contracts \
  new_bundled/catalog/entries \
  src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/src/infra/bundled/errors.rs \
  src-tauri/src/infra/bundled/entries \
  src-tauri/tests/bundled_catalog.rs
```

---

### Task 3: Add the CI-Only Full Audit and Contract-Path References

**Files:**
- Modify: `src-tauri/src/infra/bundled/catalog.rs`
- Modify: `src-tauri/tests/bundled_catalog.rs`
- Modify: `new_bundled/catalog/schemas/reference`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract`

**Interfaces:**
- Consumes: extensionless contracts/schemas/documents from Tasks 1–2.
- Produces: `pub async fn Catalog::validate(&self) -> Result<(), BundledCatalogError>` and reference identity `(contract_path, id, revision)`.

- [ ] **Step 1: Write failing audit tests**

Add to `src-tauri/tests/bundled_catalog.rs`:

```rust
#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    catalog().validate().await.unwrap();
}
```

Add one fixture-based dangling-reference test:

```rust
#[tokio::test]
async fn audit_rejects_a_dangling_reference() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join(
        "catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements",
    );
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace("\"id\": \"provisioner\"", "\"id\": \"missing\"");
    fs::write(&path, value).unwrap();

    assert!(matches!(
        Catalog::new(&temp_root).validate().await,
        Err(BundledCatalogError::UnresolvedReference {
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
```

Add one unsafe-key assertion through the public read API:

```rust
assert!(matches!(
    workflows::Entry::get(&catalog(), ("../outside", "1.0.0")).await,
    Err(BundledCatalogError::Contract { .. })
));
```

Add one fixture assertion for an unsafe path supplied by a contract:

```rust
#[tokio::test]
async fn reads_reject_an_entries_path_outside_catalog_entries() {
    let temp_root = copy_catalog_fixture();
    let path = temp_root.join("catalog/contracts/workflow_revision");
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace(
            "catalog/entries/workflows",
            "catalog/entries/../outside",
        );
    fs::write(&path, value).unwrap();

    assert!(matches!(
        workflows::Entry::all(&Catalog::new(&temp_root)).await,
        Err(BundledCatalogError::Contract { .. })
    ));

    fs::remove_dir_all(temp_root).unwrap();
}
```

- [ ] **Step 2: Run audit tests to verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
```

Expected: FAIL because `Catalog::validate` is not implemented and references still use `entity`.

- [ ] **Step 3: Migrate the shared reference schema and values**

Replace `new_bundled/catalog/schemas/reference` with:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "luma-forge://schema/reference",
  "title": "Reference",
  "type": "object",
  "additionalProperties": false,
  "required": ["contract", "id", "revision"],
  "properties": {
    "contract": {
      "type": "string",
      "pattern": "^catalog/contracts/[a-z0-9][a-z0-9_-]*$"
    },
    "id": { "type": "string", "minLength": 1 },
    "revision": { "type": "string", "minLength": 1 }
  }
}
```

Replace the four workflow reference values exactly:

| Document field | `contract` |
|---|---|
| `metadata.runtime_preset_ref` | `catalog/contracts/runtime_preset_revision` |
| `contract_requirements[].endpoint_contract_ref` | `catalog/contracts/runtime_contract_revision` |
| `contract_requirements[].provisioner_contract_ref` | `catalog/contracts/runtime_contract_revision` |
| `execution_contract.schema_ref` | `catalog/contracts/execution_schema_revision` |

Remove every `entity` field; do not retain both names.

- [ ] **Step 4: Add audit-local schema and descriptor types**

In `catalog.rs`, add:

```rust
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

`Schemas::load` must:

- read every direct file in `catalog/schemas` once;
- parse each to `Value`;
- require canonical `$id` `luma-forge://schema/<safe-name>`;
- require `<safe-name>` to equal the extensionless filename;
- reject duplicate IDs;
- build one retriever from all values;
- compile every loaded schema exactly once and store it by `$id` in `validators`; document validation reuses those compiled instances.

- [ ] **Step 5: Implement `Catalog::validate`**

Add the public method:

```rust
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
```

Add these exact helper boundaries so reads and audit share filesystem/contract parsing without sharing runtime behavior:

```rust
async fn read_all_contracts(
    &self,
) -> Result<Vec<LocatedContract>, BundledCatalogError>;

async fn audit_descriptors(
    &self,
    contracts: &[LocatedContract],
) -> Result<Vec<AuditDescriptor>, BundledCatalogError>;

async fn read_validated_references(
    &self,
    descriptor: &AuditDescriptor,
    contract: &CatalogContract,
    schemas: &Schemas,
) -> Result<Vec<LocatedReference>, BundledCatalogError>;

fn validate_references(
    references: Vec<LocatedReference>,
    known_contracts: &HashSet<&str>,
    index: &HashSet<ReferenceValue>,
) -> Result<(), BundledCatalogError>;
```

Use one recursive collector for the reserved reference shape:

```rust
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
```

Implement its helpers with the approved rules:

- contracts are every direct file in `catalog/contracts`, sorted by path;
- contract source paths become their stable identity;
- duplicate `entries_path` values fail with `Contract`;
- each contract root is traversed exactly two directory levels, sequentially;
- extra files and directories are ignored;
- each required document is read once and validated with the precompiled validator for its schema ID;
- reference scanning recursively recognizes only objects with exactly the three string fields `contract`, `id`, and `revision`;
- a reference with an unknown contract path fails with `Contract` at the source document path;
- a reference absent from the descriptor index fails with `UnresolvedReference`;
- validation never calls a concrete `Entry`, decodes a `Model`, or hydrates the target payload.

- [ ] **Step 6: Run focused tests to verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
```

Expected: PASS all read and audit integration tests.

- [ ] **Step 7: Run complete native verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected:

- all tests pass;
- formatting check exits 0;
- strict Clippy exits 0 for the completed tree.

If strict Clippy still reports only the pre-existing unused `SqliteInfraError::statement_failed` caused by user-owned SQLite repository deletions, do not modify SQLite as part of this plan. Record that external blocker and prove bundled cleanliness with:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings -A dead-code
```

- [ ] **Step 8: Commit only the audit/reference slice**

```bash
git add -f new_bundled/catalog/schemas/reference \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract \
  src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/tests/bundled_catalog.rs
git commit --only -m "feat(bundled): validate catalog integrity in CI" -- \
  new_bundled/catalog/schemas/reference \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract \
  src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/tests/bundled_catalog.rs
```

---

## Final Acceptance Checklist

- `Catalog` stores only the bundled root and has no init/cache lifecycle.
- `Entry` is a zero-sized mapping and `Model` owns loaded typed data.
- `Entry::get` reads only one revision; `Entry::all` reads only one contract root.
- Ordinary reads do not require `catalog/schemas`.
- `Catalog::validate` is the sole schema/reference integrity path.
- References use contract paths and are never hydrated automatically.
- `catalog.rs` has no concrete entry imports or entity matching.
- No `.json` files remain under `new_bundled/catalog`.
- No `entity`, `path_pattern`, `Select`, `find_by_id`, `regress`, or `walkdir` remains in the bundled implementation.
- Full tests and formatting pass; Clippy status is reported accurately.
