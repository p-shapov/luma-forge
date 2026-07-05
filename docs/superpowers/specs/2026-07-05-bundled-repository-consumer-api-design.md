# Bundled Repository Consumer API Design

## Context

This is a focused correction spec for the current isolated
`src-tauri/src/infra/bundled` layer. It supersedes the repository API,
consumer model, validation boundary, and error placement from the earlier
bundled catalog iteration without editing that historical design or plan.

The new layer remains isolated. This work does not wire bundled repositories
into `workflow_catalog`, `runtime_catalog`, app state, Tauri commands,
provider services, worker tooling, generated frontend bindings, or application
ports.

## Scope

Add stable consumer read models and direct repository lookup APIs for bundled
assets.

In scope:

- add `src-tauri/src/infra/bundled/models.rs`
- remove `src-tauri/src/infra/bundled/catalog.rs`
- remove `WorkflowRevisionPaths`
- have repositories read `generated::BUNDLED_ASSETS` directly
- have repositories return consumer DTOs from `models.rs`
- add `list` and `get(id, revision)` methods to each repository
- add RunPod workflow resolution in the bundled repository layer
- remove secret-like execution input validation from bundled catalog validation
- move `BundledValidationError` to `errors.rs`

Out of scope:

- changing old bundled catalog specs or plans
- compatibility shims for removed bundled APIs
- compatibility with old flat bundled JSON assets
- traits for single concrete repositories
- new repository tests
- wiring this repository API into old application paths

## Module Shape

Target module layout:

```text
src-tauri/src/infra/bundled/
  mod.rs
  errors.rs
  generated.rs
  models.rs
  validation.rs
  repositories/
    mod.rs
    workflows.rs
    runtime_presets.rs
    runtime_contracts.rs
    execution_schemas.rs
```

`generated.rs` remains the generated DTO and generated manifest boundary:

```rust
include!(concat!(env!("OUT_DIR"), "/bundled_types.rs"));
include!(concat!(env!("OUT_DIR"), "/bundled_manifest.rs"));
```

Generated DTOs are implementation details unless a type is explicitly exposed
through `models.rs`.

`models.rs` contains the stable consumer DTOs for future bundled consumers.
Repositories return these models instead of generated JSON shapes, paths, or
path grouping structs.

`repositories/*.rs` own direct reads from `generated::BUNDLED_ASSETS`, parse
the generated DTOs internally, and assemble consumer DTOs. There is no
intermediate `BundledCatalog` wrapper.

## Consumer Models

Keep models minimal and shaped around the data future services need. Do not
recreate old app/domain catalogs.

Required model coverage:

```rust
pub struct BundledReference {
    pub id: String,
    pub revision: String,
}

pub struct BundledWorkflow {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub runtime_preset: BundledReference,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub model_assets: Vec<BundledModelAsset>,
    pub contract_requirements: Vec<BundledWorkflowContractRequirements>,
    pub execution_contract: BundledWorkflowExecutionContract,
    pub graph: serde_json::Value,
}

pub struct BundledRuntimePreset {
    pub id: String,
    pub revision: String,
    // runtime fields from the bundled runtime preset schema
}

pub struct BundledRuntimeContract {
    pub id: String,
    pub revision: String,
    pub image_ref: String,
}

pub struct BundledExecutionSchema {
    pub id: String,
    pub revision: String,
    // inputs and outputs from the bundled execution schema
}

pub struct ResolvedRunpodWorkflow {
    pub workflow: BundledWorkflow,
    pub runtime_preset: BundledRuntimePreset,
    pub execution_schema: BundledExecutionSchema,
    pub endpoint_contract: BundledRuntimeContract,
    pub provisioner_contract: BundledRuntimeContract,
}
```

Implementation may adjust names to match existing generated schema names, but
the public repository contract must return stable consumer DTOs from
`models.rs`.

## Repository API

Each concrete repository is directly usable without reading manifest paths.

```rust
impl BundledWorkflowRepository {
    pub fn list(&self) -> Result<Vec<BundledWorkflow>, BundledCatalogError>;

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledWorkflow>, BundledCatalogError>;

    pub fn resolve_runpod_workflow(
        &self,
        id: &str,
        revision: &str,
        runtime_presets: &BundledRuntimePresetRepository,
        runtime_contracts: &BundledRuntimeContractRepository,
        execution_schemas: &BundledExecutionSchemaRepository,
    ) -> Result<Option<ResolvedRunpodWorkflow>, BundledCatalogError>;
}

impl BundledRuntimePresetRepository {
    pub fn list(&self) -> Result<Vec<BundledRuntimePreset>, BundledCatalogError>;

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimePreset>, BundledCatalogError>;
}

impl BundledRuntimeContractRepository {
    pub fn list(&self) -> Result<Vec<BundledRuntimeContract>, BundledCatalogError>;

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledRuntimeContract>, BundledCatalogError>;
}

impl BundledExecutionSchemaRepository {
    pub fn list(&self) -> Result<Vec<BundledExecutionSchema>, BundledCatalogError>;

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledExecutionSchema>, BundledCatalogError>;
}
```

`get` returns `Ok(None)` when the requested identity does not exist. Missing
lookups are not corrupt bundled assets.

`resolve_runpod_workflow` resolves only bundled references:

1. workflow revision
2. workflow runtime preset
3. workflow execution schema
4. RunPod endpoint runtime contract
5. RunPod provisioner runtime contract

If the workflow is missing, it returns `Ok(None)`. If the workflow exists but
one of its bundled references cannot be assembled, it returns
`BundledCatalogError::CorruptBundledAsset`.

The method must not import provider runtime code, create provider clients, call
application services, or wire into old app paths.

## Validation Boundary

`validation.rs` validates build-time catalog integrity only.

Keep these checks:

- approved path shape
- path identity matches JSON id and revision fields
- schema expected by path
- duplicate identities
- required workflow files
- references resolve
- required execution inputs are bound
- model asset paths are safe relative paths

Remove secret-like execution input validation from bundled validation. This
means removing the helper, cross-file validation call, and tests that reject
input IDs such as token, password, secret, credential, api_key, or apikey.

Secret-like input naming is policy, not bundled catalog structure. If that
policy is needed later, it belongs in a separate security or policy check.

## Errors

Move validation errors into `errors.rs`:

```rust
pub enum BundledCatalogError {
    CorruptBundledAsset { path: String, message: String },
}

pub(crate) enum BundledValidationError {
    Invalid { path: String, message: String },
}
```

`BundledCatalogError` remains the runtime public error surface.
`BundledValidationError` is build/test internal.

Repository parse and assembly failures map to:

```rust
BundledCatalogError::CorruptBundledAsset { path, message }
```

Repository lookup misses return `Ok(None)`.

## Testing

Do not add new repository tests for this correction.

Update existing validation tests as needed after moving
`BundledValidationError` and removing secret-like validation. If existing
repository tests fail only because `catalog.rs` and `WorkflowRevisionPaths`
were removed, delete them or make the smallest mechanical update required by
the new API. Do not add repository coverage beyond what already exists.

## Verification

Run native backend checks:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Do not run frontend codegen/build/lint unless command contracts change, which
this design explicitly avoids.
