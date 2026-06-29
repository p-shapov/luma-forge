# Rust-Side Persistence Iteration Design

## Context

This is the focused design spec for Iteration 1 of the Rust-side layer
refactor. The umbrella design is
`docs/superpowers/specs/2026-06-29-rust-side-layer-refactor-design.md`.

Iteration 1 is persistence-only. It introduces the target normalized SQLite
shape under `infra/sqlite` using SeaORM. It does not design `application`,
`runtime/runpod`, `facade`, or `composition` contracts.

## Scope

Create an isolated persistence layer:

```text
src-tauri/src/infra/sqlite/database.rs
src-tauri/src/infra/sqlite/entities/*
src-tauri/src/infra/sqlite/migrations/*
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

The complete backend does not need to remain runnable during this iteration.
Avoid compatibility shims whose only purpose is to keep old JSON persistence or
old repository contracts alive.

Out of scope:

- `application/workspace/model.rs`
- `application/workspace/ports.rs`
- `runtime/runpod/model.rs`
- `runtime/runpod/ports.rs`
- Tauri/Specta facade DTO changes
- frontend generated command bindings
- migration from the old pre-v1 JSON-backed dev database

## Dependencies

Add SeaORM and SeaORM Migration dependencies needed for SQLite persistence in
`src-tauri/Cargo.toml`. Existing SQLx dependencies may remain while old modules
still exist, but new `infra/sqlite` code uses SeaORM only.

## Schema

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
  workspace_id primary key references workspaces(id)
  datacenter_id
  gpu_id
  volume_size_gb
  network_volume_id
  provisioner_pod_id
  endpoint_id
  template_id

lifecycle_operations
  id primary key
  workspace_id references workspaces(id)
  operation_kind
  state
  created_at
  updated_at
  finished_at

runpod_operation_payloads
  operation_id primary key references lifecycle_operations(id)
  step
```

Rules:

- No `runtime_json`, `payload_json`, `meta_json`, `runtime_id`, or `payload_id`.
- Runtime identity is `workspace_id`.
- Operation payload identity is `operation_id`.
- Foreign keys are enabled.
- Provider-specific child rows use parent-owned identity and are at most one
  row per parent.
- Existing incompatible dev schemas fail clearly; no compatibility migration or
  fallback for old JSON-backed schemas is added in this iteration.

## Entities

SeaORM entities mirror the tables directly:

```text
entities/workspaces.rs
entities/runpod_workspace_runtimes.rs
entities/lifecycle_operations.rs
entities/runpod_operation_payloads.rs
```

Entities contain relational persistence shape only. They do not encode
workspace state transition rules, RunPod lifecycle step order, workflow catalog
validation, provider behavior, or UI error semantics.

## Migrations

`infra/sqlite/migrations` defines versioned SeaORM migrations for the current
target schema. The initial migration creates the normalized tables and foreign
keys listed above. It is not a compatibility migration from the old JSON-backed
development schema.

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

Workspace reads return workspace rows only. RunPod runtime reads return RunPod
runtime rows only. Lifecycle operation reads return operation rows only. RunPod
payload reads return payload rows only.

## Errors

`infra/sqlite/errors.rs` defines only technical persistence errors:

```text
SqliteInfraError
  ConnectFailed
  StatementFailed
  SchemaMismatch
  CorruptData
```

No business-shaped errors live in this iteration. Do not add `AlreadyExists`,
`RunningOperationExists`, `InvalidState`, command error codes, or UI-safe
facade errors here.

Do not derive error variants by parsing raw SQLite driver error strings.
Migration and query execution failures that do not come from explicit local
schema or stored-value parsing checks return `StatementFailed`.

Validation is limited to persistence requirements:

- primary and foreign key fields must be usable as storage keys,
- timestamps must store and parse consistently,
- numeric storage fields must fit their Rust persistence types.

Do not validate workflow existence, workspace state transitions, RunPod step
order, duplicate workspace business semantics, one-running-operation business
semantics, volume sizing, provider semantics, or UI error semantics.

## Verification

Iteration 1 does not require application, runtime, or facade behavioral tests.
Those belong in later layers. Do not add infra tests in this iteration; add
them later only when persistence behavior protects a real contract.

Use lightweight verification in the implementation plan:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
rg "#\\[cfg\\(test\\)\\]|#\\[tokio::test\\]|#\\[test\\]" src-tauri/src/infra
```

Do not add tests whose purpose is to assert removed JSON schema, removed old
repository behavior, deprecated module names, or absence of legacy
compatibility.
