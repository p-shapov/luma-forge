# Rust-Side Bundled Catalogs Iteration Design

## Scope

Iteration 2 adds `src-tauri/src/infra/bundled` as a runtime filesystem reader
for `new_bundled/catalog`. It takes an injected root path. In development this
can point at `new_bundled`.

`infra/bundled` owns:

- runtime filesystem loading from an injected catalog root path;
- reading `catalog/contracts`, `catalog/schemas`, and `catalog/entries`;
- JSON Schema validation through `jsonschema`;
- raw DTO generation through `typify`;
- catalog assembly and declarative reference resolution;
- repository adapters with `list` and `find`.

`infra/bundled` does not own:

- Tauri resource lookup or packaging wiring;
- frontend or Specta command DTO changes;
- worker execution;
- compatibility fallback to old `bundled/**`;
- migration from the old catalog shape;
- changes to existing `workflow_catalog` or `runtime_catalog`.

The old `bundled/**` directory stays untouched for compatibility, but this
iteration does not use it as a fallback.

## Source Catalog

The source of truth is `new_bundled/catalog`:

- `contracts/*.json` describes revision entities, entry path patterns, path
  params, and required files.
- `schemas/*.json` describes per-file JSON shapes.
- `entries/<catalog_type>/<name>/<version>/*.json` contains entry revision data.

References are declarative catalog data. `new_bundled/catalog/schemas/reference.json`
includes `entity`, `id`, and `revision`; entry files use that shape for
references. `entity` names the revision contract from `catalog/contracts` that
owns the referenced entry. Rust must not infer reference target entities from
field names such as `runtime_preset_ref` or `schema_ref`.

## Module Structure

```text
src-tauri/src/infra/bundled/
  mod.rs
  errors.rs
  models.rs
  generated.rs
  catalog.rs
  repositories/
    mod.rs
    workflows.rs
    runtime_contracts.rs
    runtime_presets.rs
    execution_schemas.rs
```

`catalog.rs` is the only module that reads and validates files.

`generated.rs` contains typify-generated raw DTOs from
`new_bundled/catalog/schemas/*.json`. These raw DTOs mirror schema shape and
are internal catalog input types, not the stable backend API.

`models.rs` defines stable consumer-facing backend DTOs for `infra/bundled`.
The models include workflow list data and execution data, including execution
contracts and workflow graphs. This iteration does not define how Tauri,
frontend, or later execution consumers receive those fields.

Repositories under `infra/bundled/repositories/*` wrap a loaded `Catalog`. They
expose only `list` and `find(id, revision)` methods, map raw generated DTOs into
`models.rs`, and do not read files, validate JSON, or resolve references.

## Loading And Validation

`catalog.rs` loads one in-memory `Catalog`:

1. Read `catalog/contracts/*.json`.
2. Read `catalog/schemas/*.json`.
3. Walk `catalog/entries` with `walkdir`.
4. Match revision directories against contract `path_pattern` values.
5. Require the files declared by each matched contract.
6. Parse each required file as JSON.
7. Validate each entry file with `jsonschema`.
8. Deserialize validated data into typify-generated raw DTOs.
9. Build an index of loaded revisions keyed by contract `entity`, `id`, and
   `revision`.
10. Resolve every `{ entity, id, revision }` reference found in loaded JSON
    values by selecting the matching contract descriptor from `catalog/contracts`
    and checking that the referenced revision exists in the loaded index.

Validation is limited to:

- directory and required-file rules declared by `catalog/contracts`;
- per-file shape declared by `catalog/schemas`;
- existence of referenced `{ entity, id, revision }` entries.

Do not add graph path validation, workflow execution validation, or hand-written
semantic validation unless it is first represented declaratively in
`new_bundled/catalog`.

## Errors

Errors stay small:

- `Io`
- `JsonParse`
- `Schema`
- `Contract`
- `UnresolvedReference`

Every error includes a relative catalog path when available.
`UnresolvedReference` also includes `{ entity, id, revision }`.

## Verification

No dedicated tests are required for `infra/bundled` in this iteration.

Use normal native verification during implementation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```
