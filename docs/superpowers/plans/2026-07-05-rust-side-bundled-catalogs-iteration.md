# Rust-Side Bundled Catalogs Iteration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `src-tauri/src/infra/bundled` as a runtime filesystem reader for `new_bundled/catalog`.

**Architecture:** `catalog.rs` loads and validates catalog files once from an injected root path. `generated.rs` exposes typify-generated raw DTOs, `models.rs` exposes stable backend DTOs, and `repositories/*` provide `list`/`find` over a loaded catalog. No existing `workflow_catalog`, `runtime_catalog`, Tauri command, or old `bundled/**` path is wired in this iteration.

**Tech Stack:** Rust 2021, serde/serde_json, thiserror, jsonschema, typify, schemars, syn, prettyplease, walkdir.

## Global Constraints

- Runtime catalog root is injected as a filesystem path.
- Development root can point at `new_bundled`.
- Do not add Tauri resource lookup or packaged app wiring.
- Do not change frontend or Specta command DTOs.
- Do not implement worker execution.
- Do not add compatibility fallback to old `bundled/**`.
- Do not migrate from the old catalog shape.
- Do not change existing `workflow_catalog` or `runtime_catalog`.
- Validate only contract directory rules, JSON Schema file shape, and existence of declarative `{ entity, id, revision }` references.
- Do not add graph path validation, workflow execution validation, or hand-written semantic validation unless represented declaratively in `new_bundled/catalog`.
- No dedicated tests are required for `infra/bundled` in this iteration.

---

## File Structure

- Modify `new_bundled/catalog/schemas/reference.json`: add required `entity`.
- Modify workflow entry refs:
  - `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json`
  - `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract.json`
  - `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json`
- Modify `src-tauri/Cargo.toml`: move runtime catalog crates into normal dependencies.
- Modify `src-tauri/build.rs`: generate bundled raw DTOs from schema files.
- Modify `src-tauri/src/infra/mod.rs`: export `bundled`.
- Create `src-tauri/src/infra/bundled/mod.rs`: module exports.
- Create `src-tauri/src/infra/bundled/generated.rs`: include generated raw DTOs.
- Create `src-tauri/src/infra/bundled/errors.rs`: small error enum.
- Create `src-tauri/src/infra/bundled/models.rs`: stable backend DTOs.
- Create `src-tauri/src/infra/bundled/catalog.rs`: runtime loading, schema validation, reference resolution.
- Create `src-tauri/src/infra/bundled/repositories/mod.rs`: repository exports.
- Create `src-tauri/src/infra/bundled/repositories/workflows.rs`: workflow repository.
- Create `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs`: runtime contract repository.
- Create `src-tauri/src/infra/bundled/repositories/runtime_presets.rs`: runtime preset repository.
- Create `src-tauri/src/infra/bundled/repositories/execution_schemas.rs`: execution schema repository.

---

### Task 1: Make Catalog References Self-Describing

**Files:**
- Modify: `new_bundled/catalog/schemas/reference.json`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract.json`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: existing `new_bundled/catalog/contracts/*.json` entity values.
- Produces: references shaped as `{ "entity": "runtime_contract_revision", "id": "provisioner", "revision": "1.0.0" }`.

- [ ] **Step 1: Update reference schema**

Replace `new_bundled/catalog/schemas/reference.json` with:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "luma-forge://schema/reference",
  "title": "Reference",
  "type": "object",
  "additionalProperties": false,
  "required": ["entity", "id", "revision"],
  "properties": {
    "entity": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9_]*$"
    },
    "id": {
      "type": "string",
      "pattern": "^[a-z0-9][a-z0-9_-]*$"
    },
    "revision": {
      "type": "string",
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$"
    }
  }
}
```

- [ ] **Step 2: Add entity to workflow references**

Update references in the current workflow entry:

```json
"runtime_preset_ref": {
  "entity": "runtime_preset_revision",
  "id": "comfyui-py312-cu126-torch291",
  "revision": "1.0.0"
}
```

```json
"schema_ref": {
  "entity": "execution_schema_revision",
  "id": "text-to-image",
  "revision": "1.0.0"
}
```

```json
"endpoint_contract_ref": {
  "entity": "runtime_contract_revision",
  "id": "runpod-endpoint-comfyui-hidream-o1-dev",
  "revision": "1.0.0"
}
```

```json
"provisioner_contract_ref": {
  "entity": "runtime_contract_revision",
  "id": "provisioner",
  "revision": "1.0.0"
}
```

- [ ] **Step 3: Promote runtime dependencies**

In `src-tauri/Cargo.toml`, add these to `[dependencies]`:

```toml
jsonschema = { version = "0.46.6", default-features = false }
walkdir = "2.5"
```

Leave existing `[build-dependencies]` entries for `jsonschema`, `typify`, `schemars`, `syn`, `prettyplease`, and `walkdir`; `build.rs` still needs them.

- [ ] **Step 4: Verify task**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: command compiles and existing tests pass. No new `infra/bundled` tests are added.

- [ ] **Step 5: Commit**

```bash
git add new_bundled/catalog/schemas/reference.json \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata.json \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/execution_contract.json \
  new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/contract_requirements.json \
  src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(bundled): make catalog references declarative"
```

---

### Task 2: Generate Raw DTOs And Add Bundled Module Shell

**Files:**
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/infra/mod.rs`
- Create: `src-tauri/src/infra/bundled/mod.rs`
- Create: `src-tauri/src/infra/bundled/generated.rs`

**Interfaces:**
- Consumes: schema files under `../new_bundled/catalog/schemas` relative to `src-tauri/Cargo.toml`.
- Produces: generated raw DTOs included from `OUT_DIR/bundled_generated.rs`.

- [ ] **Step 1: Generate raw DTOs in build.rs**

Replace `src-tauri/build.rs` with:

```rust
use std::{env, fs, path::PathBuf};

use schemars::schema::RootSchema;
use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    generate_bundled_types();
    tauri_build::build()
}

fn generate_bundled_types() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let schema_dir = manifest_dir
        .parent()
        .expect("repo root")
        .join("new_bundled/catalog/schemas");
    println!("cargo:rerun-if-changed={}", schema_dir.display());

    let mut schemas = fs::read_dir(&schema_dir)
        .expect("bundled schema dir should be readable")
        .map(|entry| entry.expect("bundled schema entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .collect::<Vec<_>>();
    schemas.sort();

    let mut settings = TypeSpaceSettings::default();
    settings
        .with_conversion(
            schemars::schema::SchemaObject::default(),
            "serde_json::Value",
            [typify::TypeSpaceImpl::Display].into_iter(),
        )
        .with_struct_builder(false);
    let mut type_space = TypeSpace::new(&settings);

    for schema_path in schemas {
        println!("cargo:rerun-if-changed={}", schema_path.display());
        let schema_text = fs::read_to_string(&schema_path).expect("schema should be readable");
        let root_schema: RootSchema =
            serde_json::from_str(&schema_text).expect("schema should parse");
        type_space
            .add_root_schema(root_schema)
            .expect("schema should generate Rust types");
    }

    let generated = format!(
        "{}\n{}",
        "#![allow(clippy::large_enum_variant)]",
        type_space.to_stream()
    );
    let syntax = syn::parse_file(&generated).expect("generated Rust should parse");
    let formatted = prettyplease::unparse(&syntax);
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("bundled_generated.rs");
    fs::write(out_path, formatted).expect("generated bundled DTOs should write");
}
```

- [ ] **Step 2: Export infra bundled module**

Change `src-tauri/src/infra/mod.rs` to:

```rust
pub mod bundled;
pub mod sqlite;
```

- [ ] **Step 3: Add bundled module shell**

Create `src-tauri/src/infra/bundled/mod.rs`:

```rust
pub mod generated;
```

Create `src-tauri/src/infra/bundled/generated.rs`:

```rust
include!(concat!(env!("OUT_DIR"), "/bundled_generated.rs"));
```

- [ ] **Step 4: Verify task**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: build script generates `bundled_generated.rs` and the crate compiles. `typify::TypeSpaceImpl` is re-exported by `typify` 0.7.0, so no additional build dependency is needed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/build.rs src-tauri/src/infra/mod.rs src-tauri/src/infra/bundled/mod.rs src-tauri/src/infra/bundled/generated.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(bundled): generate raw catalog DTOs"
```

---

### Task 3: Add Catalog Errors, Models, And Runtime Loader

**Files:**
- Modify: `src-tauri/src/infra/bundled/mod.rs`
- Create: `src-tauri/src/infra/bundled/errors.rs`
- Create: `src-tauri/src/infra/bundled/models.rs`
- Create: `src-tauri/src/infra/bundled/catalog.rs`

**Interfaces:**
- Consumes: `generated::*` raw DTOs and `new_bundled/catalog`.
- Produces:
  - `pub struct Catalog`
  - `impl Catalog { pub fn load(root: impl AsRef<Path>) -> Result<Self, BundledCatalogError> }`
  - `pub(crate)` raw revision accessors used by repositories.

- [ ] **Step 1: Add error enum**

Create `src-tauri/src/infra/bundled/errors.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundledCatalogError {
    #[error("bundled catalog io error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("bundled catalog json parse error at {path}: {message}")]
    JsonParse { path: String, message: String },
    #[error("bundled catalog schema error at {path}: {message}")]
    Schema { path: String, message: String },
    #[error("bundled catalog contract error at {path}: {message}")]
    Contract { path: String, message: String },
    #[error("bundled catalog unresolved reference at {path}: {entity}/{id}/{revision}")]
    UnresolvedReference {
        path: String,
        entity: String,
        id: String,
        revision: String,
    },
}
```

- [ ] **Step 2: Export catalog modules**

Change `src-tauri/src/infra/bundled/mod.rs` to:

```rust
pub mod catalog;
pub mod errors;
pub mod generated;
pub mod models;

pub use catalog::Catalog;
pub use errors::BundledCatalogError;
```

- [ ] **Step 3: Add stable models**

Create `src-tauri/src/infra/bundled/models.rs` with stable DTOs:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub entity: String,
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSchemaRevision {
    pub id: String,
    pub revision: String,
    pub inputs: Vec<ExecutionSchemaInput>,
    pub outputs: ExecutionSchemaOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchemaInput {
    pub id: String,
    pub input_type: String,
    pub required: bool,
    pub max_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchemaOutputs {
    pub output_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractRevision {
    pub id: String,
    pub revision: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetRevision {
    pub id: String,
    pub revision: String,
    pub runtime: RuntimePresetRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetRuntime {
    pub python_version: String,
    pub comfyui_revision: String,
    pub pytorch: RuntimePresetPytorch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetPytorch {
    pub index_url: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub runtime_preset_ref: Reference,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub model_assets: Vec<ModelAsset>,
    pub contract_requirements: Vec<WorkflowContractRequirement>,
    pub execution_contract: WorkflowExecutionContract,
    pub workflow_graph: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub download_source: ModelAssetSource,
    pub install_comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum ModelAssetSource {
    Huggingface {
        repository_id: String,
        file_path: String,
        revision: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkflowContractRequirement {
    Runpod {
        endpoint_contract_ref: Reference,
        provisioner_contract_ref: Reference,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionContract {
    pub schema_ref: Reference,
    pub input_bindings: Vec<InputBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBinding {
    pub value: serde_json::Value,
    pub node_id: String,
    pub path: Vec<String>,
}
```

- [ ] **Step 4: Add catalog loader types**

Create `src-tauri/src/infra/bundled/catalog.rs` with these public and internal shapes:

```rust
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

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
```

- [ ] **Step 5: Implement `Catalog::load`**

Add:

```rust
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
}
```

Implement the helper functions named in this method in the same file. Use:

- `WalkDir::new(root.join("catalog/entries")).min_depth(3).max_depth(3)` to find revision directories.
- `regress::Regex` against the relative path under `catalog/entries`, for example `workflows/comfyui-hidream-o1-dev/1.0.0`.
- `jsonschema::options().with_retriever(InMemorySchemaRetriever { schemas: schemas.clone() }).build(schema_value)` for schemas containing `luma-forge://schema/reference`.
- `serde_json::from_value::<generated::WorkflowMetadata>(value.clone())` for `metadata.json`.
- `serde_json::from_value::<generated::WorkflowModelAssets>(value.clone())` for `model_assets.json`.
- `serde_json::from_value::<generated::WorkflowContractRequirements>(value.clone())` for `contract_requirements.json`.
- `serde_json::from_value::<generated::WorkflowExecutionContract>(value.clone())` for `execution_contract.json`.
- `serde_json::from_value::<generated::WorkflowGraph>(value.clone())` for `workflow.json`.
- `serde_json::from_value::<generated::RuntimeContract>(value.clone())` for runtime contract entries.
- `serde_json::from_value::<generated::RuntimePreset>(value.clone())` for runtime preset entries.
- `serde_json::from_value::<generated::ExecutionSchema>(value.clone())` for execution schema entries.
- `serde_json::to_value` on loaded raw DTOs to walk references generically.

- [ ] **Step 6: Implement generic reference resolution**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct ReferenceValue {
    entity: String,
    id: String,
    revision: String,
}

fn resolve_references(
    root: &Path,
    contracts: &[CatalogContract],
    catalog: &Catalog,
) -> Result<(), BundledCatalogError> {
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
```

`references_in_value` recursively walks objects and yields only objects with exactly string fields `entity`, `id`, and `revision`. It must not use field names such as `runtime_preset_ref` to infer target entity.

- [ ] **Step 7: Verify task**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: compilation succeeds. Fix compile errors in `errors.rs`, `models.rs`, and `catalog.rs` before moving to Task 4.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/infra/bundled/mod.rs \
  src-tauri/src/infra/bundled/errors.rs \
  src-tauri/src/infra/bundled/models.rs \
  src-tauri/src/infra/bundled/catalog.rs
git commit -m "feat(bundled): load validated runtime catalog"
```

---

### Task 4: Add Repositories And Final Verification

**Files:**
- Create: `src-tauri/src/infra/bundled/repositories/mod.rs`
- Create: `src-tauri/src/infra/bundled/repositories/workflows.rs`
- Create: `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs`
- Create: `src-tauri/src/infra/bundled/repositories/runtime_presets.rs`
- Create: `src-tauri/src/infra/bundled/repositories/execution_schemas.rs`
- Modify: `src-tauri/src/infra/bundled/mod.rs`

**Interfaces:**
- Consumes: `Catalog` loaded by `Catalog::load`.
- Produces:
  - `WorkflowRepository::list() -> Vec<models::WorkflowRevision>`
  - `WorkflowRepository::find(id: &str, revision: &str) -> Option<models::WorkflowRevision>`
  - equivalent `list`/`find` APIs for runtime contracts, runtime presets, and execution schemas.

- [ ] **Step 1: Add repository exports**

Create `src-tauri/src/infra/bundled/repositories/mod.rs`:

```rust
pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

pub use execution_schemas::ExecutionSchemaRepository;
pub use runtime_contracts::RuntimeContractRepository;
pub use runtime_presets::RuntimePresetRepository;
pub use workflows::WorkflowRepository;
```

- [ ] **Step 2: Add repository structs**

Each repository owns a `Catalog` clone. Use this shape in each file:

```rust
use super::super::{catalog::Catalog, models};

#[derive(Debug, Clone)]
pub struct WorkflowRepository {
    catalog: Catalog,
}

impl WorkflowRepository {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Vec<models::WorkflowRevision> {
        self.catalog
            .workflows
            .iter()
            .cloned()
            .map(models::WorkflowRevision::from)
            .collect()
    }

    pub fn find(&self, id: &str, revision: &str) -> Option<models::WorkflowRevision> {
        self.catalog
            .workflows
            .iter()
            .find(|entry| entry.id == id && entry.revision == revision)
            .cloned()
            .map(models::WorkflowRevision::from)
    }
}
```

Repeat the same structure for:

- `RuntimeContractRepository` over `catalog.runtime_contracts`
- `RuntimePresetRepository` over `catalog.runtime_presets`
- `ExecutionSchemaRepository` over `catalog.execution_schemas`

- [ ] **Step 3: Add model conversions**

In `models.rs`, implement these conversions used by repositories:

- `From<crate::infra::bundled::catalog::WorkflowEntry> for WorkflowRevision`
- `From<crate::infra::bundled::catalog::RuntimeContractEntry> for RuntimeContractRevision`
- `From<crate::infra::bundled::catalog::RuntimePresetEntry> for RuntimePresetRevision`
- `From<crate::infra::bundled::catalog::ExecutionSchemaEntry> for ExecutionSchemaRevision`

```rust
impl From<crate::infra::bundled::catalog::RuntimeContractEntry> for RuntimeContractRevision {
    fn from(value: crate::infra::bundled::catalog::RuntimeContractEntry) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            image_ref: value.runtime_contract.image_ref,
        }
    }
}
```

Add equivalent conversions for workflow, runtime preset, and execution schema entries. Keep conversions mechanical: copy fields from raw generated DTOs into `models.rs`; do not add semantic validation here.

- [ ] **Step 4: Re-export repositories**

Ensure `src-tauri/src/infra/bundled/mod.rs` includes:

```rust
pub mod repositories;

pub use repositories::{
    ExecutionSchemaRepository, RuntimeContractRepository, RuntimePresetRepository,
    WorkflowRepository,
};
```

- [ ] **Step 5: Verify full native backend**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all commands pass. If `cargo fmt --check` fails, run `cargo fmt --manifest-path src-tauri/Cargo.toml`, inspect the diff, then rerun the check.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/infra/bundled src-tauri/src/infra/mod.rs src-tauri/build.rs src-tauri/Cargo.toml src-tauri/Cargo.lock new_bundled
git commit -m "feat(bundled): add catalog repositories"
```

---

## Self-Review Checklist

- Spec coverage: Tasks cover declarative refs, generated raw DTOs, `infra/bundled` module structure, runtime loader, small error payloads, repositories, and native verification.
- Scope check: Plan does not wire Tauri resources, frontend DTOs, worker execution, old `bundled/**`, `workflow_catalog`, or `runtime_catalog`.
- Placeholder scan: No deferred-work markers or unspecified validation tasks are present.
- Type consistency: Repository methods use `list` and `find(id, revision)` as required by the spec.
