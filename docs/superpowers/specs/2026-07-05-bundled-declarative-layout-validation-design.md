# Bundled Declarative Layout Validation Design

## Context

The current `src-tauri/src/infra/bundled/validation.rs` owns both bundled
filesystem structure and cross-file catalog integrity rules. This makes
directory layout changes require Rust edits even when the change is purely
declarative.

This design moves bundled directory and file structure into
`src-tauri/schemas/bundled` while keeping cross-file relationship validation in
Rust. It supersedes only the validation and identity parts of the earlier
bundled specs:

- `docs/superpowers/specs/2026-06-29-rust-side-bundled-catalogs-iteration-design.md`
- `docs/superpowers/specs/2026-07-05-bundled-repository-consumer-api-design.md`

## Scope

In scope:

- add per-entity layout specs under `src-tauri/schemas/bundled/layouts`
- add a JSON Schema for validating those layout specs
- add a shared bundled reference schema
- remove top-level entity identity fields from bundled JSON files
- derive bundled entity identity from directory paths
- update build-time validation to use `walkdir` and layout specs
- update runtime repositories to return identities derived from paths
- keep existing cross-file reference validation in Rust

Out of scope:

- changing public repository method signatures
- wiring bundled repositories into application services or Tauri commands
- adding compatibility with the old flat bundled layout
- deriving cross-entity references from paths
- creating a generic query language for relationship validation

## Sources Of Truth

`src-tauri/schemas/bundled` owns two related but separate contracts.

Entity JSON Schemas describe only JSON file contents. They must not require or
define top-level identity fields for the entity stored at that path.

Remove these own-identity fields from entity JSON and schemas:

- `id` and `revision` from `runtime_preset`, `runtime_contract`,
  `execution_schema`, and `workflow_metadata`
- `workflow_id` and `revision` from `workflow_model_assets`,
  `workflow_contract_requirements`, `workflow_execution_contract`, and
  `workflow_graph`

Reference objects remain in JSON because they represent relationships to other
bundled entities, not the identity of the current file. Add
`src-tauri/schemas/bundled/reference.schema.json` and use it from entity schemas
for:

- `workflow_metadata.runtime_preset`
- `workflow_execution_contract.schema_ref`
- `workflow_contract_requirements[].endpoint_contract`
- `workflow_contract_requirements[].provisioner_contract`

Layout specs describe filesystem shape and path-derived identity. Add:

```text
src-tauri/schemas/bundled/layout.schema.json
src-tauri/schemas/bundled/layouts/workflow_revision.layout.json
src-tauri/schemas/bundled/layouts/runtime_preset.layout.json
src-tauri/schemas/bundled/layouts/runtime_contract.layout.json
src-tauri/schemas/bundled/layouts/execution_schema.layout.json
```

Each layout spec defines:

- accepted path pattern
- named captures for entity `id` and `revision`
- expected JSON files for that entity kind
- expected `$schema` for each JSON file

## Build-Time Flow

`src-tauri/build.rs` keeps the bundled catalog build gate before
`tauri_build::build()`.

The new validation flow:

1. Load schema documents from `src-tauri/schemas/bundled/*.schema.json`.
2. Load `src-tauri/schemas/bundled/layout.schema.json`.
3. Load layout specs from `src-tauri/schemas/bundled/layouts/*.layout.json`.
4. Validate each layout spec with `jsonschema`.
5. Traverse `../bundled` with `walkdir::WalkDir::new(root).min_depth(1).sort_by_file_name()`.
6. Reject non-JSON files and unknown paths.
7. For each JSON file:
   - normalize the relative path to `/`
   - find exactly one matching layout rule
   - parse JSON
   - read `$schema`
   - require `$schema` to match the layout rule's expected schema
   - validate JSON with that entity schema
   - derive entity identity from layout path captures
   - push a `BundledAsset`
8. Run Rust cross-file validation over enriched assets.
9. Generate `OUT_DIR/bundled_manifest.rs`.

Only schemas referenced by layout file rules are bundled entity schemas. Shared
schemas such as `reference.schema.json` and validation-only schemas such as
`layout.schema.json` are available for `$ref` resolution and layout validation,
but are not treated as bundled entity types or manifest asset types.

Build `jsonschema` validators once per schema and reuse them. Resolve shared
schema references through the crate-supported in-memory registry or retriever
rather than duplicating reference definitions.

## Rust Validation Boundary

`validation.rs` remains the build-time validation boundary. It no longer owns
hardcoded bundled path shapes or path-to-schema mappings.

`BundledAsset` carries identity derived from the matching layout:

```rust
pub struct BundledAsset {
    pub path: String,
    pub schema_id: String,
    pub identity: BundledIdentity,
    pub json: serde_json::Value,
}

pub struct BundledIdentity {
    pub kind: BundledIdentityKind,
    pub id: String,
    pub revision: String,
}
```

Implementation can adjust the Rust enum names to match local style. The
identity source must be the path, not JSON entity fields.

Keep these relationship and content checks in Rust:

- duplicate identities
- workflow revision has all required files
- runtime preset references resolve
- runtime contract references resolve
- execution schema references resolve
- required execution inputs are bound
- model asset install and source paths are safe relative paths
- execution contract input binding templates are well formed

Delete the path identity matching check because the identity no longer exists
inside entity JSON.

## Runtime Repositories

`generated::BUNDLED_ASSETS` remains `&[(&str, &str)]`.

Runtime repositories parse identity from manifest paths and inject that identity
into consumer models:

- `BundledWorkflow.id`
- `BundledWorkflow.revision`
- `BundledRuntimePreset.id`
- `BundledRuntimePreset.revision`
- `BundledRuntimeContract.id`
- `BundledRuntimeContract.revision`
- `BundledExecutionSchema.id`
- `BundledExecutionSchema.revision`

Generated DTOs no longer expose the removed top-level identity fields because
the entity schemas no longer define them.

Repository `get(id, revision)` remains path-addressed. Lookup misses still
return `Ok(None)`.

## Errors

Keep the build-time validation error surface:

```rust
pub(crate) enum BundledValidationError {
    Invalid { path: String, message: String },
}
```

Error paths point to the most useful source:

- invalid layout spec: `schemas/bundled/layouts/<name>.layout.json`
- unknown bundled path: relative bundled path
- missing required workflow file: `workflows/<id>/<revision>`
- `$schema` mismatch: JSON file path
- entity schema validation failure: JSON file path
- unresolved reference: file containing the reference
- missing required input binding: `execution_contract.json`

`walkdir` traversal errors must not be skipped. Map them to
`BundledValidationError` using `walkdir::Error::path()` when available.

Runtime repositories continue to use `BundledCatalogError::CorruptBundledAsset`
for impossible post-build corruption.

## Testing

Add or update only focused tests:

- layout specs validate against `layout.schema.json`
- validation rejects unknown paths
- validation rejects paths whose `$schema` does not match the layout rule
- validation derives identity from paths
- real bundled JSON no longer contains removed top-level identity fields
- runtime repository models use path-derived identities
- existing reference validation still rejects missing references
- existing execution contract validation still rejects missing required input bindings

Do not add tests for the old flat layout, compatibility behavior, or the
absence of deprecated fields beyond the current real bundled JSON fixture check.

## Verification

Run native backend checks from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Frontend codegen, frontend build, frontend lint, and worker verification are not
required unless later implementation changes their contracts.
