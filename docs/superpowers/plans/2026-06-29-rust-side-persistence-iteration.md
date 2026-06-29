# Rust-Side Persistence Iteration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an isolated SeaORM 2.0 entity-first SQLite persistence layer
under `src-tauri/src/infra/sqlite` for normalized workspaces, runtimes,
lifecycle operations, and RunPod payload rows.

**Architecture:** Keep this iteration infra-only. SeaORM 2.0 dense entities own
schema shape and relation metadata; SeaORM schema sync owns schema creation;
concrete repositories own row-level storage. Repositories return technical
persistence errors only; no application ports, facade DTOs, legacy JSON
migration, compatibility shims, versioned migration files, or infra tests are
added.

**Tech Stack:** Rust 2021, Tauri native crate, SeaORM `2.0.0-rc.41`, SeaORM
schema sync, SQLite, existing `tokio`, `time`, and `thiserror` dependencies.

---

## Scope Boundaries

- Do not modify `application/workspace/*`, `runtime/runpod/*`, `tauri_api/*`,
  generated TypeScript bindings, frontend files, or worker files.
- Do not migrate old JSON-backed schemas.
- Do not keep old repository traits runnable through this new module.
- Do not add infra tests in this iteration.
- Do not add business errors such as `AlreadyExists`,
  `RunningOperationExists`, `InvalidState`, or UI-safe error codes.
- Do not add `runtime_json`, `payload_json`, `meta_json`, `runtime_id`, or
  `payload_id`.
- Do not classify SQLite errors by parsing raw driver error strings.
- Do not add `sea-orm-migration` or an `infra/sqlite/migrations` module.
- Do not write raw schema DDL in `infra`; schema creation must come from
  SeaORM 2.0 schema sync over entity metadata.

## File Structure

- Modify `src-tauri/Cargo.toml`: add SeaORM `2.0.0-rc.41` with SQLite,
  Tokio rustls, dense entity, entity registry, schema sync, and time features.
- Modify `src-tauri/src/lib.rs`: expose `infra`.
- Create `src-tauri/src/infra/mod.rs`: top-level infra module.
- Create `src-tauri/src/infra/sqlite/mod.rs`: exports database, entities,
  errors, model, and repositories.
- Create `src-tauri/src/infra/sqlite/errors.rs`: technical
  `SqliteInfraError` only.
- Create `src-tauri/src/infra/sqlite/model.rs`: persistence-facing structs and
  canonical timestamp encode/decode helpers.
- Create `src-tauri/src/infra/sqlite/entities/*.rs`: SeaORM 2.0 dense
  entities with relation metadata.
- Create `src-tauri/src/infra/sqlite/entities/mod.rs`: entity module exports.
- Create `src-tauri/src/infra/sqlite/database.rs`: SeaORM connection, foreign
  key enablement, schema sync execution.
- Create `src-tauri/src/infra/sqlite/repositories/mod.rs`: repository module
  exports.
- Create `src-tauri/src/infra/sqlite/repositories/workspaces.rs`: concrete
  workspace storage service.
- Create `src-tauri/src/infra/sqlite/repositories/lifecycle_operations.rs`:
  concrete lifecycle operation storage service.

---

### Task 1: Add SeaORM 2.0 Dependency and Module Shell

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/infra/mod.rs`
- Create: `src-tauri/src/infra/sqlite/mod.rs`
- Create: `src-tauri/src/infra/sqlite/entities/mod.rs`
- Create: `src-tauri/src/infra/sqlite/repositories/mod.rs`

- [ ] **Step 1: Verify toolchain**

Run:

```bash
rustc --version
```

Expected: Rust `1.94.0` or newer. If older, stop and report BLOCKED; do not
downgrade SeaORM.

- [ ] **Step 2: Add dependency**

Run:

```bash
cargo add sea-orm@2.0.0-rc.41 --manifest-path src-tauri/Cargo.toml --no-default-features --features macros,entity-registry,schema-sync,sqlx-sqlite,runtime-tokio-rustls,with-time
```

Expected:

- `src-tauri/Cargo.toml` gains `sea-orm = "2.0.0-rc.41"` with the requested
  features and `default-features = false`.
- `src-tauri/Cargo.lock` is updated.
- `sea-orm-migration` is not added.
- Existing direct `sqlx = "0.8"` may remain for old modules; do not add
  compatibility glue between SQLx versions.

- [ ] **Step 3: Expose the infra module**

In `src-tauri/src/lib.rs`, add `pub mod infra;` with the other top-level
modules.

- [ ] **Step 4: Create module files**

Create `src-tauri/src/infra/mod.rs`:

```rust
pub mod sqlite;
```

Create `src-tauri/src/infra/sqlite/mod.rs`:

```rust
pub mod database;
pub mod entities;
pub mod errors;
pub mod model;
pub mod repositories;
```

Create `src-tauri/src/infra/sqlite/entities/mod.rs`:

```rust
pub mod lifecycle_operations;
pub mod runpod_operation_payloads;
pub mod runpod_workspace_runtimes;
pub mod workspaces;
```

Create `src-tauri/src/infra/sqlite/repositories/mod.rs`:

```rust
pub mod lifecycle_operations;
pub mod workspaces;
```

- [ ] **Step 5: Run compile check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: FAIL because exported modules still do not all exist.

- [ ] **Step 6: Commit**

Run:

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/infra
git commit -m "feat(sqlite): add seaorm 2 persistence shell"
```

---

### Task 2: Add Technical Errors and Persistence Models

**Files:**

- Create: `src-tauri/src/infra/sqlite/errors.rs`
- Create: `src-tauri/src/infra/sqlite/model.rs`

- [ ] **Step 1: Create technical errors**

Create `src-tauri/src/infra/sqlite/errors.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SqliteInfraError {
    #[error("sqlite connection failed during {operation}: {message}")]
    ConnectFailed {
        operation: &'static str,
        message: String,
    },
    #[error("sqlite statement failed during {operation}: {message}")]
    StatementFailed {
        operation: &'static str,
        message: String,
    },
    #[error("sqlite schema mismatch during {operation}: {message}")]
    SchemaMismatch {
        operation: &'static str,
        message: String,
    },
    #[error("corrupt sqlite data during {operation}: {message}")]
    CorruptData {
        operation: &'static str,
        message: String,
    },
}
```

- [ ] **Step 2: Create persistence models**

Create `src-tauri/src/infra/sqlite/model.rs` with:

- `PersistedWorkspace`
- `PersistedRunpodRuntime`
- `PersistedLifecycleOperation`
- `PersistedLifecycleOperationFilter`
- `PersistedRunpodPayload`
- `format_timestamp`
- `parse_timestamp`

Timestamp helpers must store canonical UTC text with fixed-width fractional
seconds so SQLite text ordering is chronological for values written by this
layer. Use a format equivalent to:

```rust
const SQLITE_TIMESTAMP_FORMAT: &str = "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]";
```

`format_timestamp` converts to UTC before formatting and maps format failures
to `SqliteInfraError::StatementFailed`. `parse_timestamp` maps parse failures
to `SqliteInfraError::CorruptData`.

- [ ] **Step 3: Run compile check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: FAIL because entity, database, and repository modules are still
missing.

- [ ] **Step 4: Commit**

Run:

```bash
git add src-tauri/src/infra/sqlite/errors.rs src-tauri/src/infra/sqlite/model.rs
git commit -m "feat(sqlite): add persistence models"
```

---

### Task 3: Add SeaORM 2.0 Dense Entities

**Files:**

- Create: `src-tauri/src/infra/sqlite/entities/workspaces.rs`
- Create: `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`
- Create: `src-tauri/src/infra/sqlite/entities/lifecycle_operations.rs`
- Create: `src-tauri/src/infra/sqlite/entities/runpod_operation_payloads.rs`

- [ ] **Step 1: Create dense entity models**

Use SeaORM 2.0 dense entity style with `#[sea_orm::model]` where relations are
fields on `Model`.

Entity requirements:

- Every entity module imports `sea_orm::entity::prelude::*`.
- Every `Model` derives `Clone`, `Debug`, `PartialEq`, `Eq`, and
  `DeriveEntityModel`.
- Every module ends with `impl ActiveModelBehavior for ActiveModel {}`.
- Child relation fields use `HasOne<...>` with `#[sea_orm(belongs_to, from =
  "...", to = "...", on_delete = "Cascade")]`.
- Generic parent entities stay provider-polymorphic and must not expose
  provider-specific `HasOne<...>` or `HasMany<...>` child fields.
- Provider-specific child entities own `belongs_to` relations with cascade.
- `workspaces` can keep generic `HasMany<lifecycle_operations>` because
  lifecycle operations are generic, not provider-specific.
- `lifecycle_operations` does not expose a RunPod payload reverse relation.
- Relation fields are ORM metadata only and are not duplicated in
  persistence-facing structs.

Schema requirements:

- `workspaces`: string primary key `id`; string not-null `workflow_id`,
  `workflow_version`, `state`, `runtime_kind`, `created_at`, `updated_at`.
- `runpod_workspace_runtimes`: string primary key `workspace_id`; string
  not-null `datacenter_id`, `gpu_id`; integer `volume_size_gb`; nullable string
  `network_volume_id`, `provisioner_pod_id`, `endpoint_id`, `template_id`;
  belongs to `workspaces` from `workspace_id` to `id`, cascade delete.
- `lifecycle_operations`: string primary key `id`; string not-null
  `workspace_id`, `operation_kind`, `state`, `created_at`, `updated_at`;
  nullable string `finished_at`; belongs to `workspaces` from `workspace_id` to
  `id`, cascade delete.
- `runpod_operation_payloads`: string primary key `operation_id`; string
  not-null `step`; belongs to `lifecycle_operations` from `operation_id` to
  `id`, cascade delete.

Do not add `runtime_json`, `payload_json`, `meta_json`, `runtime_id`, or
`payload_id`.

- [ ] **Step 2: Compile dense relation spike**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: FAIL because database and repository modules are still missing, but
there must be no SeaORM entity derive errors.

If dense relation fields cannot express a required cascade or primary-key shape
with SeaORM 2.0, stop and report BLOCKED with the exact compiler error and the
minimal explicit SeaORM 2.0 relation metadata that would be needed. Do not
silently fall back to the old migration-owned design.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/infra/sqlite/entities
git commit -m "feat(sqlite): add dense persistence entities"
```

---

### Task 4: Add Database Connection and Schema Sync

**Files:**

- Create: `src-tauri/src/infra/sqlite/database.rs`

- [ ] **Step 1: Create database module**

Create `src-tauri/src/infra/sqlite/database.rs` with
`SqliteInfraDatabase`.

Required behavior:

- Connect to `sqlite://{path}?mode=rwc`.
- Map connection failure to `SqliteInfraError::ConnectFailed`.
- Execute `PRAGMA foreign_keys = ON`.
- Run SeaORM schema sync over only the `infra/sqlite/entities` registry.
- Map schema sync failure to `SqliteInfraError::SchemaMismatch`.
- Return a connection accessor `pub fn connection(&self) -> &DatabaseConnection`.

Use the SeaORM 2.0 registry pattern:

```rust
connection
    .get_schema_registry("luma_forge_lib::infra::sqlite::entities::*")
    .sync(&connection)
    .await
```

Adjust the prefix only if the compiler proves the crate path is different.

- [ ] **Step 2: Run compile check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: FAIL because repository modules are still missing.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/infra/sqlite/database.rs
git commit -m "feat(sqlite): sync persistence schema"
```

---

### Task 5: Add Workspace Repository

**Files:**

- Create: `src-tauri/src/infra/sqlite/repositories/workspaces.rs`

- [ ] **Step 1: Create workspace repository**

Create concrete `SqliteWorkspaceRepository<'db, C: ConnectionTrait>`.

Required public API:

```rust
pub fn new(connection: &'db C) -> Self;
pub async fn list_workspaces(&self) -> Result<Vec<PersistedWorkspace>, SqliteInfraError>;
pub async fn find_workspace(&self, id: &str) -> Result<Option<PersistedWorkspace>, SqliteInfraError>;
pub async fn insert_workspace(&self, workspace: PersistedWorkspace) -> Result<(), SqliteInfraError>;
pub async fn update_workspace(&self, workspace: PersistedWorkspace) -> Result<(), SqliteInfraError>;
pub async fn find_runpod_runtime(&self, workspace_id: &str) -> Result<Option<PersistedRunpodRuntime>, SqliteInfraError>;
pub async fn insert_runpod_runtime(&self, runtime: PersistedRunpodRuntime) -> Result<(), SqliteInfraError>;
pub async fn update_runpod_runtime(&self, runtime: PersistedRunpodRuntime) -> Result<(), SqliteInfraError>;
pub async fn delete(&self, id: &str) -> Result<(), SqliteInfraError>;
```

Implementation requirements:

- `list_workspaces` orders by `workspaces.created_at ASC`.
- `find_workspace` returns `Ok(None)` only when the workspace row is absent.
- `find_runpod_runtime` returns `Ok(None)` only when the runtime row is absent.
- Inserts and updates use SeaORM `ActiveModel` values with `Set(...)`.
- Timestamp storage uses `format_timestamp`; timestamp reads use
  `parse_timestamp`.
- SQLite statement failures return `SqliteInfraError::StatementFailed`
  directly.
- Do not add traits, DTOs, business errors, convenience APIs, compatibility
  code, or schema changes.

- [ ] **Step 2: Run compile check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: FAIL because lifecycle operation repository is still missing, or
PASS if that file exists from local edits.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/infra/sqlite/repositories/workspaces.rs
git commit -m "feat(sqlite): add workspace repository"
```

---

### Task 6: Add Lifecycle Operation Repository

**Files:**

- Create: `src-tauri/src/infra/sqlite/repositories/lifecycle_operations.rs`

- [ ] **Step 1: Create lifecycle operation repository**

Create concrete `SqliteLifecycleOperationRepository<'db, C:
ConnectionTrait>`.

Required public API:

```rust
pub fn new(connection: &'db C) -> Self;
pub async fn insert_operation(&self, operation: PersistedLifecycleOperation) -> Result<(), SqliteInfraError>;
pub async fn insert_runpod_payload(&self, payload: PersistedRunpodPayload) -> Result<(), SqliteInfraError>;
pub async fn find_operation(&self, id: &str) -> Result<Option<PersistedLifecycleOperation>, SqliteInfraError>;
pub async fn list_operations(&self, filter: Option<PersistedLifecycleOperationFilter>) -> Result<Vec<PersistedLifecycleOperation>, SqliteInfraError>;
pub async fn latest_operation(&self, workspace_id: &str) -> Result<Option<PersistedLifecycleOperation>, SqliteInfraError>;
pub async fn update_operation(&self, operation: PersistedLifecycleOperation) -> Result<(), SqliteInfraError>;
pub async fn find_runpod_payload(&self, operation_id: &str) -> Result<Option<PersistedRunpodPayload>, SqliteInfraError>;
pub async fn update_runpod_payload(&self, payload: PersistedRunpodPayload) -> Result<(), SqliteInfraError>;
pub async fn delete_for_workspace(&self, workspace_id: &str) -> Result<(), SqliteInfraError>;
```

Implementation requirements:

- `find_operation` returns `Ok(None)` only when the operation row is absent.
- `list_operations(None)` returns all lifecycle operations ordered by
  `created_at ASC`.
- `list_operations(Some(filter))` applies `workspace_id` when present and
  `states` when non-empty; state comparisons use exact strings supplied by
  callers.
- `latest_operation` filters by exact `workspace_id`, then orders by
  `created_at DESC`, `updated_at DESC`, then `id DESC`.
- `find_runpod_payload` returns `Ok(None)` only when the payload row is absent.
- Deletes use the `workspace_id` parent operation filter and rely on
  schema-sync-defined foreign-key cascade for payload rows.
- Timestamp storage uses `format_timestamp`; timestamp reads use
  `parse_timestamp`.
- SQLite statement failures return `SqliteInfraError::StatementFailed`
  directly.
- Do not add traits, DTOs, business errors, convenience APIs, compatibility
  code, or schema changes.

- [ ] **Step 2: Run compile check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: PASS.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/infra/sqlite/repositories/lifecycle_operations.rs
git commit -m "feat(sqlite): add lifecycle repository"
```

---

### Task 7: Final Verification

**Files:**

- Verify: `src-tauri/Cargo.toml`
- Verify: `src-tauri/src/lib.rs`
- Verify: `src-tauri/src/infra/sqlite/*`

- [ ] **Step 1: Run formatter**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS. If it fails, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

- [ ] **Step 2: Run crate check**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: PASS.

- [ ] **Step 3: Run native backend verification**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Confirm no raw schema bootstrap remains**

Run:

```bash
rg "CREATE TABLE|FOREIGN KEY|PRAGMA table_info|runtime_json|payload_json|meta_json|runtime_id|payload_id|AlreadyExists|RunningOperationExists|InvalidState|ConstraintViolated" src-tauri/src/infra
```

Expected: no matches. Schema DDL is generated by SeaORM 2.0 schema sync from
entity metadata, not raw SQL strings or migration builders.

- [ ] **Step 5: Confirm no infra tests were added**

Run:

```bash
rg "#\\[cfg\\(test\\)\\]|#\\[tokio::test\\]|#\\[test\\]" src-tauri/src/infra
```

Expected: no matches.

- [ ] **Step 6: Confirm migrations module is absent**

Run:

```bash
test ! -e src-tauri/src/infra/sqlite/migrations
```

Expected: PASS.

- [ ] **Step 7: Commit final cleanup if formatting changed files**

Run:

```bash
git status --short
```

Expected: either no changes, or only formatting changes from files already
touched by this plan. If there are formatting changes, commit them:

```bash
git add src-tauri/src/infra src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(sqlite): format persistence layer"
```

---

## Self-Review

- Spec coverage: SeaORM 2.0 RC dependency, dense entities, entity registry,
  schema sync, foreign-key cascades, technical errors, canonical timestamps,
  concrete repositories, no migrations, no application/runtime/facade work.
- User correction coverage: v2.0 replaces the previous v1.1 migration-owned
  schema plan; no compatibility shim is added.
- Migration decision coverage: `infra/sqlite/migrations` is intentionally
  absent; schema is owned by entity metadata and schema sync.
- Placeholder scan: no delayed implementation markers or generic validation
  instructions without concrete requirements.
- Type consistency: repository method names and persistence model names match
  the spec; workspace/runtime and operation/payload reads stay separate.
- Polymorphic parent coverage: generic parent entities do not expose
  provider-specific reverse relation fields; provider-specific child entities
  own the cascade `belongs_to` metadata.
