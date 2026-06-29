# Rust-Side Persistence Iteration Design

## Context

This is the focused design spec for Iteration 1 of the Rust-side layer
refactor. The umbrella design is
`docs/superpowers/specs/2026-06-29-rust-side-layer-refactor-design.md`.

Iteration 1 is persistence-only. It introduces the target normalized SQLite
shape under `infra/sqlite` using SeaORM 2.0 entity-first schema sync. It does
not design `application`, `runtime/runpod`, `facade`, or `composition`
contracts.

## Scope

Create an isolated persistence layer:

```text
src-tauri/src/infra/sqlite/database.rs
src-tauri/src/infra/sqlite/entities/*
src-tauri/src/infra/sqlite/repositories/*
src-tauri/src/infra/sqlite/model.rs
src-tauri/src/infra/sqlite/errors.rs
src-tauri/src/infra/sqlite/mod.rs
```

The new persistence API is infra-owned. It is not shaped by the old
`WorkspaceCatalogRepository` or `LifecycleJournalRepository` traits. The next
iteration will decide how application ports map to this persistence API.

Future application workspace ports will live in
`application/workspace/ports.rs`. Future runtime persistence ports will live in
`runtime/runpod/ports.rs`. This iteration's repositories are lower-level
SeaORM-backed storage services, not those port contracts yet. Later iterations
may add trait impls for application/runtime ports in these same repository
modules; composition will only construct and wire the concrete repositories.

The complete backend does not need to remain runnable through old persistence
contracts during this iteration. Avoid compatibility shims whose only purpose
is to keep old JSON persistence or old repository contracts alive.

Out of scope:

- `application/workspace/model.rs`
- `application/workspace/ports.rs`
- `runtime/runpod/model.rs`
- `runtime/runpod/ports.rs`
- Tauri/Specta facade DTO changes
- frontend generated command bindings
- migration from the old pre-v1 JSON-backed dev database
- versioned SeaORM migration files for this first schema

## Dependencies

Use SeaORM `2.0.0-rc.41` for the new infra module. The dependency is pinned
because SeaORM 2.0 is currently an RC line and the entity/schema-sync API
surface should not float during this refactor.

Required SeaORM features:

```text
macros
entity-registry
schema-sync
sqlx-sqlite
runtime-tokio-rustls
with-time
```

Do not add `sea-orm-migration` for this iteration. Schema creation is owned by
SeaORM 2.0 entity-first schema sync, not by hand-written migration files.

SeaORM `2.0.0-rc.41` requires Rust `1.94.0`; this repo's current local toolchain
is `rustc 1.95.0`, so the version is acceptable for this branch. SeaORM 2.0
pulls SQLx 0.9 through its own dependency graph. The existing direct `sqlx =
0.8` dependency may remain for old modules until those modules are removed; do
not add compatibility glue between the two SQLx versions.

## Schema

The canonical schema source is the SeaORM entity definitions. Database bootstrap
enables SQLite foreign keys and runs:

```rust
connection
    .get_schema_registry("luma_forge_lib::infra::sqlite::entities::*")
    .sync(&connection)
    .await
```

Use the exact crate path that compiles for the Tauri library crate. The sync
prefix must include only entities owned by `infra/sqlite`, not old SQLx-backed
modules.

Target tables:

```text
workspaces
  id primary key
  workflow_id
  workflow_version
  state
  runtime_kind
  created_at
  updated_at

runpod_workspace_runtimes
  workspace_id primary key references workspaces(id) on delete cascade
  datacenter_id
  gpu_id
  volume_size_gb
  network_volume_id
  provisioner_pod_id
  endpoint_id
  template_id

lifecycle_operations
  id primary key
  workspace_id references workspaces(id) on delete cascade
  operation_kind
  state
  created_at
  updated_at
  finished_at

runpod_operation_payloads
  operation_id primary key references lifecycle_operations(id) on delete cascade
  step
```

Rules:

- No `runtime_json`, `payload_json`, `meta_json`, `runtime_id`, or `payload_id`.
- Runtime identity is `workspace_id`.
- Operation payload identity is `operation_id`.
- Foreign keys are enabled before schema sync.
- Provider-specific child rows use parent-owned identity and are at most one
  row per parent.
- Existing incompatible dev schemas fail through technical schema-sync or
  statement errors; no compatibility migration or fallback for old JSON-backed
  schemas is added in this iteration.

## Entities

SeaORM entities mirror the tables directly:

```text
entities/workspaces.rs
entities/runpod_workspace_runtimes.rs
entities/lifecycle_operations.rs
entities/runpod_operation_payloads.rs
```

Use SeaORM 2.0 dense entity format with `#[sea_orm::model]` and relation fields
inside `Model` wherever it can express the required relation and cascade
metadata. Child entities declare `belongs_to` relations with `on_delete =
"Cascade"` so schema sync creates the target foreign keys. Parent entities use
`HasOne`/`HasMany` relation fields for ORM traversal.

If a dense relation field cannot express a required cascade or primary-key
shape, keep the entity local and explicit with SeaORM 2.0-supported relation
metadata, but do not reintroduce the old migration-owned schema design. The
acceptance condition is that schema sync creates the target tables and foreign
keys from entities.

Entities contain relational persistence shape only. They do not encode
workspace state transition rules, RunPod lifecycle step order, workflow catalog
validation, provider behavior, or UI error semantics.

## Schema Sync

`infra/sqlite` does not contain a `migrations` module. `database.rs` owns
connection bootstrap:

1. Connect to the SQLite file with `mode=rwc`.
2. Execute `PRAGMA foreign_keys = ON`.
3. Run SeaORM 2.0 schema sync for the `infra/sqlite/entities` registry.
4. Return the `DatabaseConnection`.

Schema sync is allowed to create the current target schema and to fail clearly
on incompatible existing local databases. It is not a compatibility migration
mechanism for old dev schemas.

## Persistence Models

`infra/sqlite/model.rs` defines persistence-facing structs:

```text
PersistedWorkspace {
  id,
  workflow_id,
  workflow_version,
  state,
  runtime_kind,
  created_at,
  updated_at,
}

PersistedRunpodRuntime {
  workspace_id,
  datacenter_id,
  gpu_id,
  volume_size_gb,
  network_volume_id,
  provisioner_pod_id,
  endpoint_id,
  template_id,
}

PersistedLifecycleOperation {
  id,
  workspace_id,
  operation_kind,
  state,
  created_at,
  updated_at,
  finished_at,
}

PersistedLifecycleOperationFilter {
  workspace_id,
  states,
}

PersistedRunpodPayload {
  operation_id,
  step,
}
```

Use string-backed `state`, `runtime_kind`, `operation_kind`, and `step` fields
for this iteration. Typed application/runtime enums are designed in later
iterations.

Persist timestamp columns as canonical UTC text with fixed-width fractional
seconds so SQLite text ordering matches chronological ordering for values
written by this layer.

## Repositories

Repository modules are concrete infra storage services over SeaORM entities:

```text
repositories/workspaces.rs
repositories/lifecycle_operations.rs
```

Workspace repository API:

```text
list_workspaces() -> Vec<PersistedWorkspace>
find_workspace(id) -> Option<PersistedWorkspace>
insert_workspace(workspace)
find_runpod_runtime(workspace_id) -> Option<PersistedRunpodRuntime>
insert_runpod_runtime(runpod_runtime)
update_workspace(workspace)
update_runpod_runtime(runpod_runtime)
delete(id)
```

Lifecycle operation repository API:

```text
insert_operation(operation)
insert_runpod_payload(payload)
find_operation(id) -> Option<PersistedLifecycleOperation>
list_operations(filter) -> Vec<PersistedLifecycleOperation>
latest_operation(workspace_id) -> Option<PersistedLifecycleOperation>
update_operation(operation)
find_runpod_payload(operation_id) -> Option<PersistedRunpodPayload>
update_runpod_payload(payload)
delete_for_workspace(workspace_id)
```

Write methods are row-level storage operations. Callers that write parent plus
child rows must use one repository/database transaction for the full group.
Operation filters compare exact persisted workspace IDs and state strings
supplied by callers; they do not decide which states are "running" or terminal.

## Errors

`infra/sqlite/errors.rs` defines technical persistence errors only:

```text
ConnectFailed
StatementFailed
SchemaMismatch
CorruptData
```

Repositories and database bootstrap map SeaORM/SQLite failures into these
technical errors. They do not expose raw credentials, UI-safe error codes, or
business errors such as `AlreadyExists`, `RunningOperationExists`, or
`InvalidState`. Do not classify SQLite errors by parsing raw driver error
strings in this iteration.

## Verification

This iteration does not add infra tests. Verification is compile- and
contract-based:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
rg "CREATE TABLE|FOREIGN KEY|PRAGMA table_info|runtime_json|payload_json|meta_json|runtime_id|payload_id|AlreadyExists|RunningOperationExists|InvalidState|ConstraintViolated" src-tauri/src/infra
rg "#\\[cfg\\(test\\)\\]|#\\[tokio::test\\]|#\\[test\\]" src-tauri/src/infra
```

The first four commands must pass. The two `rg` commands must return no
matches. `CREATE TABLE` and `FOREIGN KEY` are forbidden in `infra` because
schema DDL is generated by SeaORM schema sync from entity metadata.

## References

- SeaORM 2.0 package metadata: `sea-orm = 2.0.0-rc.41`
- SeaORM 2.0 Entity First Workflow / schema sync docs:
  `db.get_schema_registry("my_crate::entity::*").sync(db).await`
- SeaORM 2.0 dense entity examples in `sea-orm` crate `tests_cfg/post.rs`
