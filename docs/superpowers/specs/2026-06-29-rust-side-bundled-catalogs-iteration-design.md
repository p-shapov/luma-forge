# Rust-Side Bundled Catalogs Iteration Design

## Context

This is the focused design spec for Iteration 2 of the Rust-side layer
refactor. The umbrella design is
`docs/superpowers/specs/2026-06-29-rust-side-layer-refactor-design.md`.

Iteration 2 creates a new bundled catalog source layout and an isolated
`infra/bundled` Rust layer. It does not wire the new layer into the old catalog
modules, application ports, Tauri commands, provider flow, worker release
tooling, or frontend generated bindings.

## Scope

Move the current flat bundled assets to `old_bundled/**` and create a new
`bundled/**` tree from scratch. The new tree is ID-addressed by directory path;
there are no `catalog.json` index files.

Target source layout:

```text
old_bundled/
  workflow-catalog.json
  runtime-contracts.json
  execution-schemas.json
  runtime-presets/**
  workflows/**

bundled/
  workflows/
    <workflow_id>/<revision>/
      metadata.json
      model_assets.json
      contract_requirements.json
      execution_contract.json
      workflow.json

  runtime_presets/
    <runtime_preset_id>/<revision>.json

  runtime_contracts/
    <contract_id>/<revision>.json

  execution_schemas/
    <schema_id>/<revision>.json
```

Out of scope:

- wiring into `workflow_catalog`, `runtime_catalog`, app state, Tauri commands,
  workspace, provider, or application ports
- generated frontend bindings
- worker release tooling and worker tests
- compatibility shims for the old bundled layout

Worker release tooling may temporarily keep depending on the old layout and is
updated in a later focused task.

## JSON Schemas

Every JSON entity under the new `bundled/**` tree includes a `$schema` field.
Schemas are JSON Schema files stored under `src-tauri/schemas/bundled`.

Target schemas:

```text
src-tauri/schemas/bundled/
  workflow_metadata.schema.json
  workflow_model_assets.schema.json
  workflow_contract_requirements.schema.json
  workflow_execution_contract.schema.json
  workflow_graph.schema.json
  runtime_preset.schema.json
  runtime_contract.schema.json
  execution_schema.schema.json
```

Schemas describe individual JSON entities as stored on disk. They do not
assemble catalogs, walk directories, resolve cross-file references, or encode
runtime repository behavior.

Use `typify` to generate Rust DTOs from these JSON Schema files. Use
`jsonschema` to validate each bundled JSON entity against the schema named by
its `$schema` field.

Do not add hand-written bundled DTO mirrors. If generated types need local
aliases or re-exports, place those in `generated.rs`, not in a separate
`models.rs`.

## Rust Modules

Target module layout:

```text
src-tauri/src/infra/bundled/
  mod.rs
  catalog.rs
  errors.rs
  generated.rs
  validation.rs
  repositories/
    mod.rs
    workflows.rs
    runtime_presets.rs
    runtime_contracts.rs
    execution_schemas.rs
```

`generated.rs` includes build outputs from Cargo `OUT_DIR`:

```rust
include!(concat!(env!("OUT_DIR"), "/bundled_types.rs"));
include!(concat!(env!("OUT_DIR"), "/bundled_manifest.rs"));
```

`catalog.rs` owns bundled tree navigation and catalog assembly. It lists
generated manifest paths, groups files by approved path patterns, builds
complete workflow revision views from the five workflow files, and provides
lookup/list operations for workflow revisions, runtime presets, runtime
contracts, and execution schemas.

`repositories/*` are concrete read APIs over `catalog.rs`, following the
`infra/sqlite/repositories/*` style. Do not add traits for single concrete
repositories.

`validation.rs` owns build-time cross-file validation rules. Runtime
repositories do not call validation.

`mod.rs` exports only the concrete bundled catalog types and repository APIs
needed by this iteration.

## Build Flow

`src-tauri/build.rs` runs the bundled catalog build gate before
`tauri_build::build()`:

1. Emit `cargo::rerun-if-changed=../bundled`.
2. Emit `cargo::rerun-if-changed=schemas/bundled`.
3. Load `src-tauri/schemas/bundled/*.schema.json`.
4. Generate `OUT_DIR/bundled_types.rs` with `typify::TypeSpace`.
5. Scan `../bundled/**.json`.
6. For each bundled JSON file, read `$schema`, resolve the local schema, and
   validate the JSON entity with `jsonschema`.
7. Parse each JSON entity into the generated DTO type for its schema.
8. Run `validation.rs` cross-file validation over the parsed entities and their
   paths.
9. Generate `OUT_DIR/bundled_manifest.rs` containing static included bundled
   asset paths and contents for runtime access.
10. Run `tauri_build::build()`.

`typify` and `jsonschema` validate and parse JSON entities as-is. They do not
assemble catalogs. Catalog assembly and navigation belong to `catalog.rs`.

The validation implementation remains owned by `infra/bundled`. `build.rs` may
include that implementation through a build-script-compatible module path, but
catalog logic should not live directly in `build.rs`.

## Validation Rules

Build-time validation checks:

- every bundled JSON path matches an approved path pattern
- every workflow revision directory contains `metadata.json`,
  `model_assets.json`, `contract_requirements.json`, `execution_contract.json`,
  and `workflow.json`
- no unknown JSON files exist under `bundled/**`
- path IDs and revisions match parsed entity content
- duplicate entity identities fail
- workflow execution schema references resolve
- workflow runtime preset references resolve
- workflow runtime contract references resolve
- required execution schema inputs are bound by workflow execution contracts
- model asset install paths and model source paths are safe relative paths
- execution schema input IDs reject secret-like names such as token, password,
  secret, credential, api_key, and apikey

Runtime repositories do not rerun these rules. A successful build is the
validation boundary for bundled catalog integrity.

## Errors

Keep the public runtime error surface minimal:

```rust
pub enum BundledCatalogError {
    CorruptBundledAsset { path: String, message: String },
}
```

Runtime repository errors represent an internal corrupted bundled artifact or a
broken build contract, not user-recoverable input.

Build-time validation may produce detailed diagnostics with paths and messages,
but those diagnostics do not expand the runtime `BundledCatalogError` API.

## Testing

Add only focused behavioral tests:

- `catalog.rs` navigation and assembly over a tiny fixture manifest/tree
- concrete repositories parsing tiny fixture bundled data
- JSON Schema validation over a fixture schema and fixture JSON
- cross-file validation over a tiny fixture tree

Do not bind tests to the current real bundled catalog content. Do not add tests
for the old bundled layout, removed flat catalog files, worker release tooling,
or absence of deprecated behavior.

## Verification

Run native backend checks:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Do not run frontend codegen/build/lint for this iteration unless command
contracts change, which this design explicitly avoids. Do not run worker
verification until worker release tooling is moved to the new bundled layout.
