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
src-tauri/src/infra/sqlite/repositories/*
src-tauri/src/infra/sqlite/model.rs
src-tauri/src/infra/sqlite/errors.rs
src-tauri/src/infra/sqlite/mod.rs
```

The new persistence API is infra-owned. It is not shaped by the old
`WorkspaceCatalogRepository` or `LifecycleJournalRepository` traits. The next
iteration will decide how application ports map to this persistence API.

Future application ports will live in `application/ports.rs`. Their SQLite
implementations will stay in `infra/sqlite/repositories/*`; composition will
only construct and wire those concrete repositories. Do not create a separate
"ports implementations" layer.

The complete backend does not need to remain runnable during this iteration.
Avoid compatibility shims whose only purpose is to keep old JSON persistence or
old repository contracts alive.

Out of scope:

- `application/model.rs`
- `application/ports.rs`
- `runtime/runpod/model.rs`
- Tauri/Specta facade DTO changes
- frontend generated command bindings
- migration from the old pre-v1 JSON-backed dev database

## Dependencies

Add SeaORM dependencies needed for SQLite persistence in `src-tauri/Cargo.toml`.
Existing SQLx dependencies may remain while old modules still exist, but new
`infra/sqlite` code uses SeaORM only.

## Schema

Target tables:

```text
workspaces
  id primary key
  workflow_id
  workflow_version
  state
  created_at
  updated_at

workspace_runtimes
  workspace_id primary key references workspaces(id)
  runtime_kind

runpod_workspace_runtimes
  workspace_id primary key references workspace_runtimes(workspace_id)
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
- Child rows are strict 1:1 with parent rows.
- Existing incompatible dev schemas fail clearly; no migration or fallback is
  added in this iteration.

## Entities

SeaORM entities mirror the tables directly:

```text
entities/workspaces.rs
entities/workspace_runtimes.rs
entities/runpod_workspace_runtimes.rs
entities/lifecycle_operations.rs
entities/runpod_operation_payloads.rs
```

Entities contain relational persistence shape only. They do not encode
workspace state transition rules, RunPod lifecycle step order, workflow catalog
validation, provider behavior, or UI error semantics.

## Persistence Models

`infra/sqlite/model.rs` defines persistence-facing structs:

```text
PersistedWorkspace {
  id,
  workflow_id,
  workflow_version,
  state,
  created_at,
  updated_at,
}

PersistedWorkspaceRuntime {
  workspace_id,
  runtime_kind,
}

PersistedRunpodWorkspaceRuntime {
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

PersistedRunpodOperationPayload {
  operation_id,
  step,
}
```

Use string-backed `state`, `runtime_kind`, `operation_kind`, and `step` fields
for this iteration. Typed application/runtime enums are designed in later
iterations.

## Repositories

Repository modules:

```text
repositories/workspaces.rs
repositories/lifecycle_operations.rs
```

Workspace repository API:

```text
list() -> Vec<(PersistedWorkspace, PersistedRunpodWorkspaceRuntime)>
find(id) -> Option<(PersistedWorkspace, PersistedRunpodWorkspaceRuntime)>
insert_runpod(workspace, runtime)
update_runpod(workspace, runtime)
delete(id)
```

Lifecycle operation repository API:

```text
create(operation)
find_running_by_workspace(workspace_id)
list_running()
latest_for_workspace(workspace_id)
update(operation, payload)
mark_state(operation_id, state, payload)
delete_for_workspace(workspace_id)
```

Repository methods should avoid business interpretation. For absence, prefer
`Option` or affected-row counts over domain-style errors. Parent and child
writes happen in one transaction.

Reads reconstruct parent plus RunPod child persistence records. If required
child rows are missing, return a technical corrupt-data or schema error.

## Errors

`infra/sqlite/errors.rs` defines only technical persistence errors:

```text
SqliteInfraError
  ConnectFailed
  StatementFailed
  TransactionFailed
  ConstraintViolated
  SchemaMismatch
  CorruptData
```

No business-shaped errors live in this iteration. Do not add `AlreadyExists`,
`RunningOperationExists`, `InvalidState`, command error codes, or UI-safe
facade errors here.

If SQLite reports a unique, foreign key, or check constraint failure, return
`ConstraintViolated` with technical context such as operation, table, or
constraint name when available. Later application code can map constraints to
business outcomes if needed.

Validation is limited to persistence requirements:

- primary and foreign key fields must be usable as storage keys,
- timestamps must store and parse consistently,
- parent/child rows needed to reconstruct a persistence record must exist,
- numeric storage fields must fit their Rust persistence types.

Do not validate workflow existence, workspace state transitions, RunPod step
order, duplicate workspace business semantics, one-running-operation business
semantics, volume sizing, provider semantics, or UI error semantics.

## Verification

Iteration 1 does not require new behavioral tests. Behavior belongs in higher
layers: application use cases, RunPod runtime sequencing, and facade
integration. Persistence tests at this stage should not lock in unnecessary
implementation details.

Use lightweight verification in the implementation plan:

```text
review SeaORM entities against the approved table/column contract
review repository methods for transaction boundaries
review that repository methods return technical infra errors only
review that no secret-bearing fields are introduced
run the narrowest compile/check command that is practical after the edit
```

A small schema smoke test may be added if it is cheaper than manual inspection,
but it is not required by this spec.

Do not add tests whose purpose is to assert removed JSON schema, removed old
repository behavior, deprecated module names, or absence of legacy
compatibility.
