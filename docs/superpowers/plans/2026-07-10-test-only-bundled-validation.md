# Test-Only Bundled Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the production `Catalog::validate` API and compile the complete catalog audit only as part of the `bundled_catalog` integration-test target.

**Architecture:** `catalog.rs` remains a private production storage engine with no validation or test-support API. `src-tauri/tests/bundled_catalog/validation.rs` independently owns contract/schema parsing, descriptor/reference auditing, path safety, and private audit errors. All observable read and audit tests remain in `src-tauri/tests/bundled_catalog.rs`.

**Tech Stack:** Rust 2021, Tokio filesystem APIs, serde/serde_json, thiserror, jsonschema 0.46.6 as a dev-dependency.

## Global Constraints

- `Catalog` stores only the bundled root and exposes no validation method or audit lifecycle.
- Declare validation with `#[path = "bundled_catalog/validation.rs"] mod validation;` from the integration test target.
- Keep all 12 observable read and audit tests in `src-tauri/tests/bundled_catalog.rs`.
- Keep ordinary reads unchanged, including contract validation, safe paths, symlink rejection, sequential I/O, and relative errors.
- Keep schema values, compiled validators, descriptors, and reference indexes local to one audit.
- Use integration-test-private `ValidationError`; remove `Schema` and `UnresolvedReference` from public `BundledCatalogError`.
- Move `jsonschema` to dev-dependencies. Keep `regress`, which Typify-generated runtime DTOs require.
- Do not add production test hooks, caches, concurrency, registries, compatibility code, or dependencies.
- Do not edit generated Rust output manually.
- Preserve unrelated SQLite and user-owned changes. Stage only named files and use a Conventional Commit.

---

### Task 1: Make the Bundled Audit Integration-Test-Only

**Files:**
- Create: `src-tauri/tests/bundled_catalog/validation.rs`
- Modify: `src-tauri/tests/bundled_catalog.rs`
- Modify: `src-tauri/src/infra/bundled/catalog.rs`
- Modify: `src-tauri/src/infra/bundled/errors.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify only if Cargo changes it: `src-tauri/Cargo.lock`

**Interfaces:**
- Produces: `validation::validate(root: &Path) -> Result<(), ValidationError>`, visible only to the parent integration-test module.
- Removes: `Catalog::validate`, runtime `jsonschema`, and public audit-only error variants.
- Preserves: the four public `Entry::all`/`Entry::get` APIs and all existing observable test cases.

- [ ] **Step 1: Wire a missing test-only validator and verify RED**

Add to `src-tauri/tests/bundled_catalog.rs`:

```rust
#[path = "bundled_catalog/validation.rs"]
mod validation;

use validation::{validate, ValidationError};
```

Change the packaged audit test to call the not-yet-created function:

```rust
#[tokio::test]
async fn packaged_catalog_passes_full_audit() {
    validate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"))
        .await
        .unwrap();
}
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog packaged_catalog_passes_full_audit
```

Expected: FAIL because `validation::validate` does not exist.

- [ ] **Step 2: Remove audit responsibilities from production**

Delete these items from `src-tauri/src/infra/bundled/catalog.rs`:

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
read_direct_files
all jsonschema references
```

Keep the production boundary private. Do not change these items to `pub(super)` or `pub(crate)`:

```text
CatalogContract
RequiredFile
RevisionDescriptor
Catalog::read_contract
Catalog::revision_descriptors
is_safe_component
read_required_json
read_json_value
relative_to_root
```

After deletion, production imports remain:

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

- [ ] **Step 3: Implement the integration-test validator**

Create `src-tauri/tests/bundled_catalog/validation.rs`. Its externally used boundary is exactly:

```rust
pub(super) async fn validate(root: &Path) -> Result<(), ValidationError>;

#[derive(Debug, thiserror::Error)]
pub(super) enum ValidationError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Json {
        path: String,
        source: serde_json::Error,
    },
    Contract {
        path: String,
        message: String,
    },
    Schema {
        path: String,
        message: String,
    },
    UnresolvedReference {
        path: String,
        contract: String,
        id: String,
        revision: String,
    },
}
```

Define these private audit data types in the same file:

```text
CatalogContract { entries_path, required_files }
RequiredFile { name, schema }
RevisionDescriptor { id, revision, path }
SchemaRetriever
Schemas { values, validators }
ReferenceValue { contract, id, revision }
LocatedReference { path, value }
LocatedContract { path, contract }
AuditDescriptor { contract_index, id, revision, path }
```

Both contract structs use `#[serde(deny_unknown_fields)]`. `ReferenceValue` derives `Clone`, `Debug`, `Eq`, `Hash`, `PartialEq`, and `Deserialize`.

Implement these functions inside the test module:

```rust
impl Schemas {
    async fn load(root: &Path) -> Result<Self, ValidationError>;
}

async fn read_all_contracts(root: &Path) -> Result<Vec<LocatedContract>, ValidationError>;

async fn audit_descriptors(
    root: &Path,
    contracts: &[LocatedContract],
) -> Result<Vec<AuditDescriptor>, ValidationError>;

async fn revision_descriptors(
    root: &Path,
    contract: &CatalogContract,
) -> Result<Vec<RevisionDescriptor>, ValidationError>;

async fn read_validated_references(
    root: &Path,
    descriptor: &AuditDescriptor,
    contract: &CatalogContract,
    schemas: &Schemas,
) -> Result<Vec<LocatedReference>, ValidationError>;

fn collect_references(value: &Value, output: &mut Vec<ReferenceValue>);

fn validate_references(
    references: Vec<LocatedReference>,
    known_contracts: &HashSet<&str>,
    index: &HashSet<ReferenceValue>,
) -> Result<(), ValidationError>;
```

The audit sequence remains exact:

1. Load sorted direct contracts and reject duplicate `entries_path` values.
2. Load every schema once, require filename/`$id` identity, and compile each validator once with one retriever.
3. Require every contract schema before entry traversal, including contracts with zero revisions.
4. Traverse exactly `<entries_path>/<id>/<revision>` sequentially and build the contract-path index.
5. Read each required document once, reuse the compiled validator, and recursively collect only exact three-field references.
6. Reject unknown contract paths and unresolved `(contract, id, revision)` targets.

Implement local path-safe helpers rather than production hooks:

```rust
fn validate_contract(
    contract: &CatalogContract,
    contract_path: &str,
) -> Result<(), ValidationError>;
fn validate_entries_path(path: &str) -> Result<(), String>;
fn is_safe_component(value: &str) -> bool;
async fn read_directories(
    root: &Path,
    directory: &Path,
) -> Result<Vec<(String, PathBuf)>, ValidationError>;
async fn read_direct_files(
    root: &Path,
    directory: &Path,
) -> Result<Vec<PathBuf>, ValidationError>;
async fn read_required_json(root: &Path, path: &Path) -> Result<Value, ValidationError>;
async fn read_json_value(root: &Path, path: &Path) -> Result<Value, ValidationError>;
async fn reject_symlinks(root: &Path, path: &Path) -> Result<(), ValidationError>;
fn symlink_error(root: &Path, path: &Path) -> ValidationError;
fn relative_to_root(root: &Path, path: &Path) -> String;
```

These helpers preserve the production contract/path rules and reject symlinks in both discovery and selected paths. All error paths remain relative to `root`.

- [ ] **Step 4: Keep all audit assertions in `bundled_catalog.rs`**

Replace only audit calls and audit-specific error matches:

```rust
validate(&temp_root).await

Err(ValidationError::Schema { path, .. })

Err(ValidationError::UnresolvedReference {
    contract,
    id,
    revision,
    ..
})

Err(ValidationError::Contract { path, .. })
```

Keep these 12 test names in `src-tauri/tests/bundled_catalog.rs`:

```text
catalog_construction_performs_no_io
packaged_catalog_passes_full_audit
entry_mappings_read_owned_models
get_reads_only_the_selected_revision_without_schemas
get_rejects_a_missing_selected_document
get_rejects_unsafe_keys_as_catalog_contract_errors
audit_rejects_a_dangling_reference
audit_rejects_a_missing_contract_schema_without_revisions
reads_reject_a_retired_contract_field
audit_and_reads_reject_a_symlinked_contract
audit_and_reads_reject_a_symlinked_revision
reads_reject_an_entries_path_outside_catalog_entries
```

- [ ] **Step 5: Remove public audit errors and the runtime dependency**

`BundledCatalogError` retains only:

```rust
pub enum BundledCatalogError {
    Io { path: String, source: std::io::Error },
    Json { path: String, source: serde_json::Error },
    Contract { path: String, message: String },
    Entry { path: String, message: String },
}
```

Move the unchanged dependency declaration from `[dependencies]` to:

```toml
[dev-dependencies]
jsonschema = { version = "0.46.6", default-features = false }
```

Keep `regress = "0.11.1"` under normal dependencies.

- [ ] **Step 6: Run focused GREEN and production-boundary checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test bundled_catalog
cargo check --manifest-path src-tauri/Cargo.toml --lib
rg -n 'Catalog::validate|pub async fn validate|BundledCatalogError::(Schema|UnresolvedReference)' src-tauri/src src-tauri/tests
cargo tree --manifest-path src-tauri/Cargo.toml --edges normal | rg 'jsonschema'
```

Expected:

- 12/12 bundled catalog integration tests pass;
- the production library compiles without validation code;
- both `rg` commands print no matches (exit 1 expected).

- [ ] **Step 7: Run complete native verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

If strict Clippy fails only on the unchanged `SqliteInfraError::statement_failed`, run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings -A dead-code
```

Expected: tests and formatting pass; the fallback passes with all non-dead-code warnings denied.

- [ ] **Step 8: Commit the implementation slice**

Run:

```bash
git diff --check
git status --short
git add src-tauri/src/infra/bundled/catalog.rs \
  src-tauri/src/infra/bundled/errors.rs \
  src-tauri/tests/bundled_catalog.rs \
  src-tauri/tests/bundled_catalog/validation.rs \
  src-tauri/Cargo.toml
```

Add `src-tauri/Cargo.lock` only if changed. Commit:

```bash
git commit -m "refactor(bundled): make catalog validation test-only"
```
