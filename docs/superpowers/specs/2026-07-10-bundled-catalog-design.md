# Bundled Catalog Design

## Goal

Build `src-tauri/src/infra/bundled` as a small, typed storage engine for the immutable JSON objects under `new_bundled/catalog`.

The design treats each Rust entry module like a SeaORM entity module:

- `Entry` describes how a record type maps to bundled storage;
- `Model` contains owned, typed data loaded from storage;
- a JSON contract describes the physical layout and required documents;
- `Catalog` provides generic runtime I/O without knowing concrete entry modules;
- a module owned by the bundled catalog integration-test target audits the complete packaged catalog in CI.

The design is intentionally incompatible with the current implementation. There are no legacy aliases, fallback paths, or compatibility tests.

## Public API

`Catalog` stores only the bundled root. Construction performs no I/O and cannot fail.

```rust
pub struct Catalog {
    root: PathBuf,
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self;
}
```

Each module under `bundled/entries` exposes a zero-sized mapping type and an owned model:

```rust
pub struct Entry;

pub struct Model {
    pub id: String,
    pub revision: String,
    // Typed documents owned by this entry type.
}
```

Concrete entry modules expose direct read operations:

```rust
let workflows = workflows::Entry::all(&catalog).await?;

let workflow = workflows::Entry::get(
    &catalog,
    ("workflow-id", "1.0.0"),
).await?;
```

Required behavior:

```rust
impl Entry {
    pub async fn all(
        catalog: &Catalog,
    ) -> Result<Vec<Model>, BundledCatalogError>;

    pub async fn get(
        catalog: &Catalog,
        key: (&str, &str),
    ) -> Result<Option<Model>, BundledCatalogError>;
}
```

There is no query builder, `Select`, `find`, `find_by_id`, `one`, initialization lifecycle, or cache lifecycle.

## Entry Mapping Contract

Concrete entries implement one internal generic mapping contract:

```rust
trait CatalogEntry {
    type Model;

    const CONTRACT: &'static str;

    fn decode(
        id: String,
        revision: String,
        documents: Documents,
    ) -> Result<Self::Model, BundledCatalogError>;
}
```

`CONTRACT` must name one direct file inside `catalog/contracts`. The same path is the stable identity used by references and the full-audit index.

Example:

```rust
impl CatalogEntry for workflows::Entry {
    type Model = workflows::Model;

    const CONTRACT: &'static str =
        "catalog/contracts/workflow_revision";

    fn decode(
        id: String,
        revision: String,
        mut documents: Documents,
    ) -> Result<Self::Model, BundledCatalogError> {
        Ok(workflows::Model {
            id,
            revision,
            metadata: documents.take("metadata")?,
            workflow: documents.take("workflow")?,
        })
    }
}
```

`Documents` owns parsed JSON values and the relative revision path used for errors. Its typed `take` operation deserializes one named document into an existing generated schema type.

The entry module does not repeat an entity identifier, entries root, required-file list, or schema identifiers.

## Bundled Layout

Every object under `new_bundled/catalog` is JSON. Files therefore have no `.json` extension.

```text
new_bundled/
└── catalog/
    ├── contracts/
    │   ├── execution_schema_revision
    │   ├── runtime_contract_revision
    │   ├── runtime_preset_revision
    │   └── workflow_revision
    ├── schemas/
    │   ├── reference
    │   ├── workflow_metadata
    │   └── ...
    └── entries/
        └── workflows/
            └── <id>/
                └── <revision>/
                    ├── metadata
                    ├── model_assets
                    ├── contract_requirements
                    ├── execution_contract
                    └── workflow
```

All entry types use the fixed physical layout:

```text
<entries_path>/<id>/<revision>
```

`id` and `revision` are opaque UTF-8 directory names. The storage engine only requires each value to be one safe, normal path component. Semver and domain-specific identity rules belong above the storage layer.

Uncontracted files and directories are ignored. Validation asserts positive requirements from contracts and does not reject extra content.

## Contract Format

A contract is identified by its own path relative to the bundled root. It contains the entries root and required documents:

```json
{
  "entries_path": "catalog/entries/workflows",
  "required_files": [
    {
      "name": "metadata",
      "schema": "luma-forge://schema/workflow_metadata"
    },
    {
      "name": "workflow",
      "schema": "luma-forge://schema/workflow_graph"
    }
  ]
}
```

The contract contains no `entity`, regex, path template, or named capture configuration.

Contract invariants:

- `entries_path` is a safe relative path inside `catalog/entries`;
- each required-file name is one safe, normal path component;
- required-file names are unique within the contract;
- required schema values are canonical `luma-forge://schema/<name>` JSON Schema `$id` URIs;
- two loaded contracts cannot declare the same `entries_path`.

## Schema Validation

Schema files remain standard JSON Schemas with canonical string `$id` values. Cross-schema `$ref` values resolve through those IDs.

Schema identity maps directly to storage without a manifest:

```text
luma-forge://schema/workflow_metadata
→ catalog/schemas/workflow_metadata
```

The `<name>` suffix must be one safe, normal path component. The loaded schema's root `$id` must equal the URI used to address it.

Ordinary `Entry::get` and `Entry::all` reads do not access `catalog/schemas` and do not perform JSON Schema validation. They still parse JSON and deserialize it into generated Rust types, so missing files, malformed JSON, and incompatible document shapes remain runtime errors.

Schema and reference integrity for the trusted bundled data is enforced by a validation module compiled only as part of the `bundled_catalog` integration-test target. The module is not part of the production build or public API. During its audit, one operation-local schema set owns the loaded schemas and compiled validators:

```rust
struct Schemas {
    values: HashMap<String, Value>,
    validators: HashMap<String, jsonschema::Validator>,
}
```

Within one audit:

- each schema `$id` is read and parsed at most once;
- each schema required for document validation is compiled at most once;
- every revision using the same schema reuses the compiled validator;
- schema values, retriever state, and validators are dropped when the audit completes.

This reuse is audit-local only. It does not introduce state into `Catalog` or create a cache lifecycle. An invalid schema fails the CI audit but does not affect an ordinary read.

No schema cache, watcher, refresh API, or interior mutability is introduced.

Registry invariants:

- every schema has a canonical string `$id` whose `<name>` suffix matches its filename;
- `$id` values are unique;
- every schema directly or transitively referenced by a contract exists;
- schema compilation and document validation errors include the relative document or schema path.

The existing schema type generation remains separate from runtime reading. Entry models compose the generated document types explicitly; contract-driven model code generation and macros are out of scope. The `jsonschema` crate is a dev-dependency because only the test-only audit uses it.

## References

Catalog references use the same contract path identity as Rust entry mappings:

```json
{
  "contract": "catalog/contracts/runtime_contract_revision",
  "id": "provisioner",
  "revision": "1.0.0"
}
```

There is no separate entity identifier.

An object with exactly the string fields `contract`, `id`, and `revision` is a catalog reference value. The shared reference JSON Schema defines this reserved shape.

References are not checked or hydrated by ordinary reads. An entry model retains the typed reference value exactly as stored. Full reference integrity is checked only by the test-only CI audit.

A port implementation explicitly loads a referenced model through its known target entry type when a use case needs it:

```rust
let runtime = runtime_contracts::Entry::get(
    &catalog,
    (&reference.id, &reference.revision),
).await?;
```

The consumer is responsible for requiring the expected `reference.contract` before using a concrete target entry type. The catalog does not perform runtime dispatch from contract paths to Rust entry types, recursively hydrate object graphs, or hide additional filesystem reads behind model decoding.

## Read Data Flow

`Entry::get` performs the following work:

1. validate that `Entry::CONTRACT` is a safe relative path and that `id` and `revision` are safe single components;
2. load and parse the exact contract named by `Entry::CONTRACT`;
3. verify that the contract's entries root exists;
4. address `<entries_path>/<id>/<revision>` directly;
5. return `Ok(None)` when that revision directory does not exist;
6. read and parse only the selected contract's required documents;
7. pass the owned documents to `Entry::decode` for typed deserialization;
8. return an owned model and drop all query-local metadata and JSON values.

`Entry::all` performs the same work but enumerates directory entries exactly two levels below the selected contract's `entries_path`. It considers directory entries only, ignores extra files, and sorts revision descriptors by `id` then `revision` before reading and returning their models.

Required files are read directly. There is no preliminary metadata/existence call. A read returning `NotFound` becomes a missing-required-file contract error; other filesystem failures remain I/O errors.

An ordinary read does not load schemas or other contracts, traverse other entries roots, build a global revision index, validate references, or retain any cache.

## Test-Only Full Catalog Audit

`src-tauri/tests/bundled_catalog/validation.rs` contains an integration-test-private asynchronous fail-fast audit function:

```rust
async fn validate(root: &Path) -> Result<(), ValidationError>;
```

1. load every direct file in `catalog/contracts` and record its relative contract path;
2. load every direct schema file and build the audit-local schema registry;
3. validate every contract and reject duplicate `entries_path` values;
4. enumerate each declared `<entries_path>/<id>/<revision>` directory;
5. build a query-local index keyed by `(contract_path, id, revision)`;
6. read and schema-validate every required document for every indexed revision;
7. collect catalog reference values from validated documents;
8. require every referenced contract path to be loaded;
9. require every referenced `(contract_path, id, revision)` tuple to exist in the index;
10. return the first error or `Ok(())`, then drop all audit state.

Each direct contract filename must be one safe UTF-8 component; its stable identity is constructed as `catalog/contracts/<name>` without lossy path conversion.

`src-tauri/tests/bundled_catalog.rs` declares the module with `#[path = "bundled_catalog/validation.rs"] mod validation;`. The integration target contains one smoke test against the packaged `new_bundled` tree; all other audit behavior uses purpose-built temporary fixtures. Runtime startup and production builds do not contain or invoke the audit.

## Error Model

The public error type stays small and path-aware:

```rust
pub enum BundledCatalogError {
    Io { path: String, source: std::io::Error },
    Json { path: String, source: serde_json::Error },
    Contract { path: String, message: String },
    Entry { path: String, message: String },
}
```

Schema, reference, I/O, JSON, and contract audit failures use a private `ValidationError` in the integration-test validation module, without adding audit-only variants or support hooks to the public API.

Only paths relative to the bundled root appear in errors. Absolute host paths are never exposed.

Error semantics:

- an absent revision requested through `get` is `Ok(None)`;
- an absent required document is a `Contract` error;
- invalid JSON is a `Json` error;
- typed document/model decoding failures are `Entry` errors;
- unsafe storage paths and malformed contracts are `Contract` errors;

The private validation error distinguishes schema failures from unresolved references for focused audit assertions.

There are no fallback paths or aggregate error-reporting mode.

## Module Boundaries

```text
src-tauri/src/infra/bundled/
├── catalog.rs
├── codegen.rs
├── entries/
│   ├── mod.rs
│   ├── execution_schemas.rs
│   ├── runtime_contracts.rs
│   ├── runtime_presets.rs
│   └── workflows.rs
├── errors.rs
├── generated.rs
└── mod.rs

src-tauri/tests/
├── bundled_catalog.rs
├── bundled_catalog/
│   └── validation.rs
└── fixtures/
    └── bundled_catalog/
        └── catalog/
```

Responsibilities:

- `catalog.rs`: root-bound engine, contract and document loading, generic runtime reads, storage invariants, and unit tests through a fake `CatalogEntry`;
- `entries/mod.rs`: internal `CatalogEntry` contract and `Documents`;
- concrete entry modules: `Entry`, `Model`, direct public reads, explicit decoding;
- `codegen.rs` and `generated.rs`: schema-derived raw document types;
- `errors.rs`: the path-aware public runtime error type;
- `tests/bundled_catalog.rs`: one packaged-catalog audit smoke and one public concrete-entry mapping test;
- `tests/bundled_catalog/validation.rs`: integration-test-only audit implementation plus audit unit tests using minimal temporary catalog trees;
- `tests/fixtures/bundled_catalog`: stable minimal documents for the four concrete public entry mappings, independent of packaged workflow and preset inventory.

`catalog.rs` may depend on the generic `CatalogEntry` contract but must not import or match on concrete entry modules. Adding a new entry type requires a contract, schemas/documents, and one entry module; it does not require editing `catalog.rs`.

## Tests

Use three test layers:

- `catalog.rs` unit tests use a fake `CatalogEntry` and programmatically-created temporary trees for construction, `all`/`get`, absent revisions, selected-read isolation, missing documents, unsafe keys/paths, retired contract fields, and symlink rejection;
- audit unit tests live beside the test-only validation implementation and use a minimal temporary contract/schema/reference fixture for dangling references, missing schemas, unsafe contract identities, and symlinks;
- `bundled_catalog.rs` keeps exactly one smoke audit against the real `new_bundled` tree and one concrete-entry mapping test against the stable test fixture.

Do not copy the packaged catalog into behavioral fixtures. Do not mock the filesystem or add a filesystem abstraction; path and symlink behavior must use real temporary directories. Changes to packaged workflow, preset, contract, or entry inventory must affect only the one explicit packaged-catalog smoke test.

Do not add tests for removed vocabulary or APIs, ignored extra content, private helpers, exact error prose, or identical cases repeated for every entry module.

## Removed Design

The refactor deletes rather than preserves:

- `Catalog::init` and eager payload aggregation;
- `CatalogEntry::ENTITY`;
- contract `entity` and `path_pattern` fields;
- regex matching and named `id`/`revision` captures;
- `RevisionDescriptor.entity`;
- global index construction during ordinary reads;
- query-time reference validation;
- automatic reference hydration and runtime entry dispatch;
- runtime schema loading, dependency resolution, compilation, and validation;
- the public `Catalog::validate` audit API;
- `Select`, `PhantomData`, `find`, `find_by_id`, `all/one` builder composition;
- the combined mapping-and-data `Entry` type;
- `.json` filename extensions under `new_bundled/catalog`;
- compatibility aliases, legacy fixtures, and tests for removed behavior.

## Non-goals

- application port or adapter implementations;
- domain-model mapping;
- writes or mutations to bundled storage;
- caching, refresh, or filesystem watching;
- concurrent bulk reads;
- runtime entry registration or plugins;
- automatic relationship loading;
- arbitrary entry path layouts;
- domain validation of IDs or revisions;
- generating composite entry models from contracts;
- user-authored presets, external catalogs, and their import-time validation boundary.
