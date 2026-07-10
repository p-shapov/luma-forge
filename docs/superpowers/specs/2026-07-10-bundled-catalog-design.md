# Bundled Catalog Design

## Goal

Build `src-tauri/src/infra/bundled` as a small, typed storage engine for the immutable JSON objects under `new_bundled/catalog`.

The design treats each Rust entry module like a SeaORM entity module:

- `Entry` describes how a record type maps to bundled storage;
- `Model` contains owned, typed data loaded from storage;
- a JSON contract describes the physical layout and required documents;
- `Catalog` provides generic I/O, schema validation, and full-catalog auditing without knowing concrete entry modules.

The design is intentionally incompatible with the current implementation. There are no legacy aliases, fallback paths, or compatibility tests.

## Public API

`Catalog` stores only the bundled root. Construction performs no I/O and cannot fail.

```rust
pub struct Catalog {
    root: PathBuf,
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self;

    pub async fn validate(
        &self,
    ) -> Result<(), BundledCatalogError>;
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

`Documents` owns schema-validated JSON values and the relative revision path used for errors. Its typed `take` operation deserializes one named document into an existing generated schema type.

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

## Schema Registry

Schema files remain standard JSON Schemas with canonical string `$id` values. Cross-schema `$ref` values resolve through those IDs.

Schema identity maps directly to storage without a manifest:

```text
luma-forge://schema/workflow_metadata
→ catalog/schemas/workflow_metadata
```

The `<name>` suffix must be one safe, normal path component. The loaded schema's root `$id` must equal the URI used to address it.

An ordinary read loads only the schema dependency closure needed by its selected contract:

1. start with schema IDs named by the contract's required files;
2. load each corresponding schema file;
3. recursively collect non-local `$ref` URI bases from the loaded schema;
4. load unseen referenced schemas until the closure is complete;
5. reject unsupported external schema namespaces;
6. build an in-memory retriever from the resulting closure.

Local fragment references beginning with `#` do not load another file. Cycles terminate through a set of already loaded schema IDs.

Each operation owns one query-local schema set:

```rust
struct Schemas {
    values: HashMap<String, Value>,
    validators: HashMap<String, jsonschema::Validator>,
}
```

Within one `get`, `all`, or `validate` operation:

- each schema `$id` in the dependency closure is read and parsed at most once;
- each schema required for document validation is compiled at most once;
- every revision using the same schema reuses the compiled validator;
- schema values, retriever state, and validators are dropped when the operation completes.

This reuse is operation-local only. It does not introduce state into `Catalog` or create a cache lifecycle.

The full audit loads every direct schema file and validates its identity, then uses the same dependency rules. An invalid unrelated schema therefore fails `Catalog::validate` but does not affect an ordinary read.

No schema cache, watcher, refresh API, or interior mutability is introduced.

Registry invariants:

- every schema has a canonical string `$id` whose `<name>` suffix matches its filename;
- `$id` values are unique;
- every schema directly or transitively referenced by a selected contract exists;
- schema compilation and document validation errors include the relative document or schema path.

The existing schema type generation remains separate from runtime reading. Entry models compose the generated document types explicitly; contract-driven model code generation and macros are out of scope.

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

References are not checked by ordinary reads. Full reference integrity is checked only by `Catalog::validate`.

## Read Data Flow

`Entry::get` performs the following work:

1. validate that `Entry::CONTRACT` is a safe relative path and that `id` and `revision` are safe single components;
2. load and parse the exact contract named by `Entry::CONTRACT`;
3. load the query-local schema dependency closure required by that contract;
4. verify that the contract's entries root exists;
5. address `<entries_path>/<id>/<revision>` directly;
6. return `Ok(None)` when that revision directory does not exist;
7. read only the selected contract's required documents;
8. parse and schema-validate each document;
9. pass the owned validated documents to `Entry::decode`;
10. return an owned model and drop all query-local metadata and JSON values.

`Entry::all` performs the same work but enumerates directory entries exactly two levels below the selected contract's `entries_path`. It considers directory entries only, ignores extra files, and sorts revision descriptors by `id` then `revision` before reading and returning their models.

Required files are read directly. There is no preliminary metadata/existence call. A read returning `NotFound` becomes a missing-required-file contract error; other filesystem failures remain I/O errors.

An ordinary read does not load other contracts, traverse other entries roots, build a global revision index, validate references, or retain any cache.

## Full Catalog Audit

`Catalog::validate` is an explicit asynchronous fail-fast operation:

1. load every direct file in `catalog/contracts` and record its relative contract path;
2. load every direct schema file and validate its complete dependency closure;
3. validate every contract and reject duplicate `entries_path` values;
4. enumerate each declared `<entries_path>/<id>/<revision>` directory;
5. build a query-local index keyed by `(contract_path, id, revision)`;
6. read and schema-validate every required document for every indexed revision;
7. collect catalog reference values from validated documents;
8. require every referenced contract path to be loaded;
9. require every referenced `(contract_path, id, revision)` tuple to exist in the index;
10. return the first error or `Ok(())`, then drop all audit state.

The packaged `new_bundled` catalog is audited by one integration test in CI. Runtime startup and ordinary reads do not invoke the full audit automatically.

## Error Model

The public error type stays small and path-aware:

```rust
pub enum BundledCatalogError {
    Io { path: String, source: std::io::Error },
    Json { path: String, source: serde_json::Error },
    Contract { path: String, message: String },
    Schema { path: String, message: String },
    Entry { path: String, message: String },
    UnresolvedReference {
        path: String,
        contract: String,
        id: String,
        revision: String,
    },
}
```

Only paths relative to the bundled root appear in errors. Absolute host paths are never exposed.

Error semantics:

- an absent revision requested through `get` is `Ok(None)`;
- an absent required document is a `Contract` error;
- invalid JSON is a `Json` error;
- schema loading, reference resolution, compilation, and document validation failures are `Schema` errors;
- typed document/model decoding failures are `Entry` errors;
- unsafe storage paths and malformed contracts are `Contract` errors;
- dangling references are `UnresolvedReference` errors.

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
```

Responsibilities:

- `catalog.rs`: root-bound engine, contract/schema loading, generic reads, full audit;
- `entries/mod.rs`: internal `CatalogEntry` contract and `Documents`;
- concrete entry modules: `Entry`, `Model`, direct public reads, explicit decoding;
- `codegen.rs` and `generated.rs`: schema-derived raw document types;
- `errors.rs`: the path-aware public error type.

`catalog.rs` may depend on the generic `CatalogEntry` contract but must not import or match on concrete entry modules. Adding a new entry type requires a contract, schemas/documents, and one entry module; it does not require editing `catalog.rs`.

## Tests

Keep tests at observable boundaries:

- constructing `Catalog` performs no I/O;
- `Entry::all` returns owned models for one entry type;
- `Entry::get` returns the selected model;
- `Entry::get` returns `None` for an absent revision;
- a broken sibling revision does not affect `Entry::get`;
- an invalid unrelated schema does not affect `Entry::get`;
- a missing or invalid selected document fails at read time;
- one compact test exercises mappings for the four existing entry modules;
- one integration test runs `Catalog::validate` against the real `new_bundled` tree;
- the audit rejects one dangling contract reference;
- unsafe relative paths are rejected.

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
- arbitrary entry path layouts;
- domain validation of IDs or revisions;
- generating composite entry models from contracts.
