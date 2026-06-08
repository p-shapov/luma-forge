# Provisioned Remote Lifecycle Refactor Design

## Problem

The native layer currently models provisioned remote workspace lifecycle through `provisioned_remote_compute` services and workspace snapshot state. Provisioning and cleanup mutate external provider resources, but the durable local state is not structured as an operation journal. If the app stops between a provider mutation and a workspace update, native can lose precise knowledge of the operation step that was in progress.

This refactor stabilizes the provisioned remote lifecycle before adding more provider or runtime features.

## Goals

- Rename `provisioned_remote_compute` to `provisioned_remote`.
- Move provisioned remote runtime-specific domain types out of the generic workspace domain.
- Add a durable, provisioned-remote-specific operation journal.
- Keep workspace root persistence generic while allowing runtime-owned persistence tables.
- Keep commands thin adapters over native services.
- Remove the current cancellation model from this refactor.
- Preserve granular native errors through specific error codes, without adding a generic error context object.
- Keep the design compatible with future background operations without implementing them now.

## Non-Goals

- No generic `RuntimeOperation` abstraction.
- No generic runtime persistence registry.
- No second provider support.
- No automatic retries.
- No mid-flight cancellation.
- No background task runner in this refactor.
- No compatibility migration for existing pre-v1 persisted workspace data.
- No frontend UI redesign.

## Module Structure

The runtime-specific boundary becomes:

```text
src-tauri/src/provisioned_remote/
  mod.rs
  errors.rs
  service.rs
  flow.rs
  cleanup.rs
  contracts.rs
  helpers.rs
  coordination.rs
  provider.rs
  registry.rs
  test_support.rs

  journal/
    mod.rs
    repository.rs
    sqlite.rs
    schema.rs

  store/
    mod.rs
    repository.rs
    sqlite.rs
    schema.rs

  providers/
    mod.rs
    runpod/
      mod.rs
      api.rs
      mapping.rs
      config.rs
      provisioner.rs
```

Provisioned remote runtime domain types live under the existing domain module:

```text
src-tauri/src/domain/provisioned_remote.rs
```

The generic workspace domain stays small:

```rust
pub enum WorkspaceRuntime {
    ProvisionedRemote(ProvisionedRemoteRuntime),
}

pub struct Workspace {
    pub id: String,
    pub workflow_preset: WorkflowPreset,
    pub runtime: WorkspaceRuntime,
}
```

Provisioned remote-specific runtime state and operation domain types move into `domain/provisioned_remote.rs`.

## SQLite Boundary

Add a small shared SQLite database/bootstrap owner:

```text
src-tauri/src/sqlite/
  mod.rs
  database.rs
  schema.rs
```

`SqliteNativeDatabase` owns the `SqlitePool`, bootstraps each schema module, and exposes cloned pools to repositories. It has no business logic.

Schema bootstrap calls:

```text
workspace_catalog::schema::bootstrap(pool)
provisioned_remote::store::schema::bootstrap(pool)
provisioned_remote::journal::schema::bootstrap(pool)
```

This creates one shared SQLite database while keeping table ownership in the module that owns the data.

## Persistence Model

The generic workspace table stores only runtime-neutral workspace root data:

```text
workspaces
- id
- runtime_type
- workflow_preset_json
- created_at
- updated_at
```

Provisioned remote runtime data lives in its own table:

```text
provisioned_remote_runtimes
- workspace_id
- runtime_json
- active_operation_id nullable
- created_at
- updated_at
```

The operation journal uses two tables:

```text
provisioned_remote_operations
- id
- workspace_id
- kind
- status
- operation_json
- created_at
- updated_at
- finished_at nullable
```

```text
provisioned_remote_operation_steps
- id
- operation_id
- sequence
- kind
- status
- step_json
- created_at
- updated_at
- finished_at nullable
```

Normalize only the columns needed for lifecycle correctness:

- operation identity
- workspace identity
- operation kind
- operation status
- step sequence
- step kind
- step status

Detailed safe payloads stay in JSON. The journal must not store raw provider request bodies, raw provider response bodies, provider API keys, Hugging Face keys, worker tokens, credential-bearing URLs, SDK debug output, or environment dumps.

Useful constraints and indexes:

```text
provisioned_remote_runtimes.workspace_id primary key
provisioned_remote_operations.workspace_id index
provisioned_remote_operations.status index
provisioned_remote_operation_steps.operation_id index
unique(provisioned_remote_operation_steps.operation_id, sequence)
```

The service owns invariants that cross tables.

## Domain Model

Replace the existing `ProvisionedRemoteComputeProvisioningState`, `ProvisionedRemoteComputeProvisioningStatus`, and `ProvisionedRemoteComputeProvisioningPhase` concepts with smaller app-facing runtime status plus detailed operation steps.

Runtime status:

```rust
pub enum ProvisionedRemoteRuntimeStatus {
    NotProvisioned,
    Provisioning { summary: ProvisionedRemoteProgressSummary },
    Ready,
    CleaningUp { summary: ProvisionedRemoteProgressSummary },
    CleanupRequired { reason: ProvisionedRemoteCleanupRequiredReason },
    Invalid { error: ProvisionedRemoteRuntimeError },
}
```

Progress summary:

```rust
pub struct ProvisionedRemoteProgressSummary {
    pub operation_id: ProvisionedRemoteOperationId,
    pub kind: ProvisionedRemoteOperationKind,
    pub current_step: Option<ProvisionedRemoteOperationStepKind>,
    pub percent: Option<u8>,
}
```

Operation status:

```rust
pub enum ProvisionedRemoteOperationStatus {
    Running,
    Completed,
    Failed { error: ProvisionedRemoteOperationError },
    Stale { reason: ProvisionedRemoteStaleReason },
}
```

Step status:

```rust
pub enum ProvisionedRemoteOperationStepStatus {
    Running,
    Completed,
    Failed { error: ProvisionedRemoteOperationError },
}
```

Operation kinds:

```rust
pub enum ProvisionedRemoteOperationKind {
    Provision,
    Cleanup,
}
```

Provision step kinds:

```text
CreateVolume
StartProvisioner
PollProvisioner
TerminateProvisioner
CreateEndpoint
```

Cleanup step kinds:

```text
DeleteEndpoint
TerminateProvisioner
DeleteVolume
```

There is no `Cancelling` state in the new model.

## Service Responsibilities

`ProvisionedRemoteService` is the behavior owner for the provisioned remote runtime. It coordinates:

- workspace root repository and root-row persistence
- provisioned remote runtime store
- operation journal repository
- workflow catalog
- provider registry
- provider flow and cleanup helpers

Commands remain adapters:

- accept typed request DTOs
- call the service
- map returned domain state to binding-safe responses
- map service errors to `NativeCommandError`

Provider adapters remain under `provisioned_remote/providers/runpod/` and return only UI-safe snapshots and errors.

## Create Workspace Flow

`create_workspace(request)` creates both the generic workspace root row and the provisioned remote runtime row.

Flow:

```text
1. Resolve the workflow preset from the workflow catalog.
2. Build the initial ProvisionedRemoteRuntime state with status=NotProvisioned and empty resources.
3. Start transaction.
4. Insert workspaces root row:
   - id
   - runtime_type=provisioned_remote
   - workflow_preset_json
   - created_at
   - updated_at
5. Insert provisioned_remote_runtimes runtime row:
   - workspace_id
   - runtime_json
   - active_operation_id=null
   - created_at
   - updated_at
6. Commit transaction.
7. Return the assembled Workspace.
```

Workspace root persistence is part of the provisioned remote service flow because creating this runtime requires writing both the root aggregate data and the runtime-owned row atomically.

## Provision Flow

`provision_workspace(workspace_id)` runs synchronously in this refactor.

Flow:

```text
1. Start transaction.
2. Load workspace root and provisioned remote runtime row.
3. Reject if active_operation_id is set or a running operation exists.
4. Create operation: kind=provision, status=running.
5. Set active_operation_id on provisioned_remote_runtimes.
6. Set runtime status=Provisioning(summary).
7. Update the generic workspaces.updated_at timestamp because the workspace aggregate changed.
8. Commit transaction.
9. Execute steps synchronously:
   - CreateVolume
   - StartProvisioner
   - PollProvisioner
   - TerminateProvisioner
   - CreateEndpoint
10. Before each provider call or poll, insert a running step.
11. After provider success or failure, transactionally update the step, operation, provisioned remote runtime row, active_operation_id, and workspaces.updated_at when needed.
12. Return the assembled Workspace.
```

Provider calls happen outside database transactions.

`TerminateProvisioner` remains part of the provision operation because the provisioner pod is transient and must be cleaned up after provisioning succeeds or fails.

## Cleanup Flow

`cleanup_workspace(workspace_id)` also runs synchronously.

Flow:

```text
1. Start transaction.
2. Load workspace root and provisioned remote runtime row.
3. Reject if active_operation_id is set.
4. Create operation: kind=cleanup, status=running.
5. Set active_operation_id.
6. Set runtime status=CleaningUp(summary).
7. Update the generic workspaces.updated_at timestamp because the workspace aggregate changed.
8. Commit transaction.
9. Execute cleanup steps:
   - DeleteEndpoint if endpoint exists.
   - TerminateProvisioner if provisioner exists.
   - DeleteVolume if volume exists.
10. Insert a running step before each provider call.
11. Treat expected not-found provider errors as successful cleanup for that resource.
12. On success, transactionally clear resources, set status=NotProvisioned, clear active_operation_id, mark operation completed, and update workspaces.updated_at.
13. On failure, transactionally preserve remaining resource snapshots, clear active_operation_id, mark operation failed, and update workspaces.updated_at. Set runtime status to CleanupRequired when any remote resource snapshot remains. If no remote resource snapshot remains, set runtime status to Invalid.
14. Return the assembled Workspace.
```

## Startup Stale Handling

During app bootstrap, after database connection and schema bootstrap:

```text
1. Query provisioned_remote_operations where status=running.
2. For each running operation:
   - mark operation status=Stale(AppInterrupted)
   - mark running step rows status=Failed with an OperationInterrupted error in step_json
   - load the related provisioned remote runtime row
   - clear active_operation_id
   - set runtime status to Invalid(OperationInterrupted) or CleanupRequired if resource snapshots remain
   - update workspaces.updated_at
3. Commit updates transactionally per affected workspace/operation.
```

No provider calls happen during startup.

## Cancellation And Future Background Operations

The new model removes `cancel_workspace_provisioning` from the current command surface.

Mid-flight cancellation is deferred because synchronous command execution does not provide a clean cancellation mechanism. If a user needs to stop after a failed, stale, or partial provision, the app should call `cleanup_workspace`.

Future cancellation requires introducing a background operation executor. That future work should add:

- background task registry keyed by operation ID
- cancellation tokens
- command API split into start/status/cancel
- event emission or polling
- background-safe transaction boundaries
- startup reconciliation for interrupted background tasks

The operation journal is intentionally compatible with that future. It stores durable operation state independently of whether the executor is synchronous or background.

## Error Handling

Keep `NativeCommandError` simple:

```rust
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
}
```

Do not add a generic error context object.

Granularity should come from specific error codes and runtime status values, not from a growing catch-all context payload.

Add or rename native codes only when app behavior needs to differ, for example:

```text
ProvisionedRemoteOperationAlreadyRunning
ProvisionedRemoteOperationStale
ProvisionedRemoteProvisionFailed
ProvisionedRemoteCleanupFailed
ProvisionedRemoteCleanupRequired
ProviderUnauthorized
ProviderRateLimited
ProviderTimeout
ProviderRequestFailed
ProvisionerWorkerUnauthorized
ProvisionerWorkerUnavailable
ProvisionerWorkerResponseInvalid
```

Detailed safe failure information may be preserved in operation or step JSON payloads and in workspace runtime status. Runtime invalidity is represented as `ProvisionedRemoteRuntimeStatus::Invalid`, while operation and step execution failures remain represented as `Failed`. No failure payload may leak secrets or raw provider payloads.

## Command Boundary

Target workspace commands:

```text
create_workspace
provision_workspace
cleanup_workspace
get_workspace_catalog
```

Remove or defer:

```text
cancel_workspace_provisioning
```

Responses should use the renamed runtime language:

```text
WorkspaceRuntimeResponse::ProvisionedRemote(...)
ProvisionedRemoteRuntimeStatusResponse
ProvisionedRemoteProgressSummaryResponse
```

Do not add standalone operation history commands in this refactor. The journal exists for native correctness, not as a user-facing history feature.

## Testing

Add or update native tests for:

- provisioned remote domain status transitions
- operation creation
- duplicate running-operation rejection
- provision step sequencing
- cleanup step sequencing
- provider failure marking step and operation failed
- workspace runtime invalid or cleanup-required state
- preserving resources after cleanup failure
- treating expected not-found cleanup errors as success
- startup stale handling
- no provider calls during startup stale handling
- SQLite schema bootstrap for root workspace, provisioned remote runtime, operations, and steps
- create workspace writes root and provisioned remote runtime rows transactionally
- workspace root plus provisioned remote runtime row round trips
- missing runtime row treated as corrupt or schema invalid
- running operation query without JSON parsing
- step ordering by sequence
- transactional workspace/runtime/operation updates
- command error mappings to granular stable codes
- generated bindings reflect `ProvisionedRemote` naming and no cancellation command

Verification commands:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

If command contracts change:

```bash
bun run codegen:commands
bun run build
bun run lint
```

## Implementation Constraints

- Use direct composition with the provisioned remote store; do not add a generic runtime store registry yet.
- Keep provider trait redesign narrow. Rename existing traits to `ProvisionedRemote*`, but do not generalize for a second provider.
- Existing pre-v1 workspace database contents can be discarded rather than migrated.
- Service-owned transactions should coordinate cross-table consistency. Repositories should support transaction-aware methods where atomic multi-table updates are required.
