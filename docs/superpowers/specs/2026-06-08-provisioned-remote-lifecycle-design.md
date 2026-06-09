# Provisioned Remote Lifecycle Refactor Design

## Problem

The native layer currently models provisioned remote workspace lifecycle through `provisioned_remote_compute` services and workspace snapshot state. Provisioning and cleanup mutate external provider resources, but the durable local state is not structured as a lifecycle operation journal. If the app stops between a provider mutation and a workspace update, native can lose precise knowledge of the lifecycle step that was in progress.

This refactor stabilizes the provisioned remote lifecycle before adding more provider or runtime features.

## Goals

- Rename `provisioned_remote_compute` to `provisioned_remote`.
- Move provisioned remote runtime-specific domain types out of the generic workspace domain.
- Add a durable generic lifecycle operation journal with provisioned remote payload support.
- Keep workspace persistence generic while storing runtime-specific workspace data in a single workspace row.
- Keep commands thin adapters over native services.
- Remove the current cancellation model from this refactor.
- Preserve granular native errors through specific error codes, without adding a generic error context object.
- Run provisioned remote lifecycle operations through a native background executor.
- Keep lifecycle progress notifications Tauri-agnostic inside the service layer.

## Non-Goals

- No generic `RuntimeOperation` abstraction.
- No generic runtime persistence registry.
- No second provider support.
- No automatic retries.
- No mid-flight cancellation.
- No compatibility migration for existing pre-v1 persisted workspace data.
- No frontend UI redesign.
- No lifecycle operation resume after app restart.
- No automatic provider cleanup during app startup.

## Module Structure

The runtime-specific boundary becomes:

```text
src-tauri/src/provisioned_remote/
  mod.rs
  errors.rs
  service.rs
  contracts.rs
  events.rs
  provider.rs
  registry.rs
  test_support.rs

  lifecycle/
    mod.rs
    provision.rs
    cleanup.rs
    delete.rs
    background.rs
    coordination.rs
    helpers.rs

  providers/
    mod.rs
    runpod/
      mod.rs
      api.rs
      mapping.rs
      config.rs
      provisioner.rs
```

Workspace aggregate persistence lives in one table. Runtime-specific workspace JSON handling is delegated through explicit runtime implementations:

```text
src-tauri/src/workspace_catalog/
  mod.rs
  repository.rs
  sqlite.rs
  schema.rs
  runtime.rs

  runtimes/
    mod.rs
    provisioned_remote.rs
```

Lifecycle operation persistence lives at the native layer root because `LifecycleOperation` is the client-facing cross-runtime envelope. Runtime-specific payload handling is delegated through explicit payload implementations:

```text
src-tauri/src/lifecycle_journal/
  mod.rs
  repository.rs
  sqlite.rs
  schema.rs
  payload.rs

  payloads/
    mod.rs
    provisioned_remote.rs
```

Generic lifecycle operation domain types live under the existing domain module:

```text
src-tauri/src/domain/lifecycle_operation.rs
```

Provisioned remote runtime resource and lifecycle operation payload types live under the existing domain module:

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
    pub state: WorkspaceState,
    pub runtime: WorkspaceRuntime,
}
```

Provisioned remote-specific runtime resource, lifecycle operation payload, and step types move into `domain/provisioned_remote.rs`.

The `lifecycle/` submodule owns provision, cleanup, delete, and background operation execution. Keep the term "job" available for future `execute_workspace` workflow execution concepts, such as generated image jobs and worker job payloads.

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
lifecycle_journal::schema::bootstrap(pool)
```

This creates one shared SQLite database while keeping table ownership in the module that owns the data.

## Persistence Model

The generic workspace table stores the workspace aggregate in one row:

```text
workspaces
- id
- runtime_type
- provider_id
- state
- workflow_preset_json
- runtime_json
- created_at
- updated_at
```

The root `lifecycle_journal` module uses one generic table:

```text
lifecycle_operations
- id
- workspace_id
- state
- payload_json
- created_at
- updated_at
- finished_at nullable
```

Normalize only the columns needed for lifecycle correctness:

- runtime provider identity
- workspace state discriminator
- lifecycle operation identity
- workspace identity
- lifecycle operation state

`state` stores stable workspace conditions: `not_provisioned`, `ready`, `cleanup_required`, or `invalid`. `runtime_json` stores the runtime-specific snapshot, including remote placement and resource snapshots. It must not duplicate workspace state.

In-flight lifecycle work is represented by running rows in the lifecycle operation journal, not by workspace states such as `provisioning` or `cleaning_up`, and not by a runtime-owned active operation pointer.

`workspace_catalog/runtimes/provisioned_remote.rs` owns provisioned remote runtime serialization, validation, and provider identity derivation. It does not own workspace state or a separate table in this refactor.

`payload_json` stores the runtime-specific lifecycle operation payload, including the operation-kind discriminator and current step. It must not duplicate `workspace_id` or `runtime_type`; the runtime discriminator comes from the related workspace row. The repository reconstructs the client-facing `LifecycleOperation` aggregate from the workspace row, lifecycle operation row, and payload. Lifecycle steps are operation-specific enum values, not standalone entities in this refactor.

`lifecycle_journal/payloads/provisioned_remote.rs` owns provisioned remote payload serialization, validation, and operation-kind decoding from `payload_json`. It does not own a separate table in this refactor.

Detailed safe payloads stay in JSON. The journal must not store raw provider request bodies, raw provider response bodies, provider API keys, Hugging Face keys, worker tokens, credential-bearing URLs, SDK debug output, or environment dumps.

Useful constraints and indexes:

```text
workspaces.id primary key
workspaces.runtime_type index
workspaces.provider_id index
workspaces.state index
lifecycle_operations.workspace_id index
lifecycle_operations.state index
unique running lifecycle operation per workspace
```

The unique running lifecycle operation constraint should be implemented as a partial unique index over `lifecycle_operations(workspace_id)` where `state = 'running'`.

The service owns invariants that cross workspace and lifecycle journal tables. The workspace repository owns writing the workspace row as one workspace aggregate persistence operation.

## Domain Model

Replace the existing `ProvisionedRemoteComputeProvisioningState`, `ProvisionedRemoteComputeProvisioningStatus`, and `ProvisionedRemoteComputeProvisioningPhase` concepts with stable workspace state plus detailed lifecycle operations.

Workspace state:

```rust
pub enum WorkspaceState {
    NotProvisioned,
    Ready,
    CleanupRequired { reason: WorkspaceCleanupRequiredReason },
    Invalid { reason: WorkspaceRuntimeInvalidReason },
}
```

Workspace cleanup-required reasons:

```rust
pub enum WorkspaceCleanupRequiredReason {
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    OperationInterrupted,
}
```

Workspace runtime errors:

```rust
pub enum WorkspaceRuntimeInvalidReason {
    OperationInterrupted,
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    CorruptRuntimeState,
}
```

Detailed diagnostic logging and error tracing are out of scope for this refactor. Generic workspace state should not store stack traces, provider-specific error taxonomies, arbitrary provider messages, worker environment data, raw provider payloads, credentials, or credential-bearing URLs. Keep workspace failure states coarse and stable so the system can later add a dedicated sanitized logging/tracing pipeline without changing workspace lifecycle semantics.

Runtime:

```rust
pub struct ProvisionedRemoteRuntime {
    pub placement: RemotePlacementPlan,
    pub resources: ProvisionedRemoteResources,
}
```

Lifecycle operation:

```rust
pub struct LifecycleOperation {
    pub operation_id: LifecycleOperationId,
    pub workspace_id: WorkspaceId,
    pub state: LifecycleOperationState,
    pub payload: LifecycleOperationPayload,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub finished_at: Option<DateTime>,
}
```

The lifecycle operation kind is persisted inside `payload_json` as part of the runtime-specific payload enum. The domain model uses `LifecycleOperationPayload` as the client-facing runtime discriminator and does not store a separate mutable kind field.

Lifecycle operation state:

```rust
pub enum LifecycleOperationState {
    Running,
    Completed,
    Failed,
    Stale,
}
```

Lifecycle operation state records execution status only. Operation kind, current step, and runtime-specific failure detail live in the runtime-specific payload.

Lifecycle operation payload:

```rust
pub enum LifecycleOperationPayload {
    ProvisionedRemote(ProvisionedRemoteLifecycleOperationPayload),
}
```

Provisioned remote lifecycle operation payload:

```rust
pub enum ProvisionedRemoteLifecycleOperationPayload {
    Provision {
        step: Option<ProvisionedRemoteProvisionStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Cleanup {
        step: Option<ProvisionedRemoteCleanupStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Delete {
        step: Option<ProvisionedRemoteDeleteStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
}
```

Provisioned remote lifecycle error:

```rust
pub enum ProvisionedRemoteLifecycleError {
    AppInterrupted,
    ProviderAdapterUnavailable,
    ProviderSecretUnavailable,
    ProviderApiFailed { reason: ProviderApiError },
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    InvalidRuntimeState,
}
```

`ProvisionerUnavailable` and `ProvisionerResponseInvalid` are native client-side provisioner interaction failures. `ProvisionerFailed` means the worker reported a terminal failure. Do not persist provisioner error codes, messages, or context in this refactor; worker failure codes are future sanitized logging/diagnostics material.

`ProviderApiError` remains the provider-facing error category and is nested as `ProviderApiFailed { reason }`. The payload error is UI-safe runtime detail; it must not include raw provider payloads, arbitrary provider response bodies, credentials, credential-bearing URLs, SDK debug output, or worker environment data.

Provider API error:

```rust
pub enum ProviderApiError {
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
}
```

`RequestFailed` is intentionally message-free in persisted state and frontend responses. Provider adapters may keep internal diagnostic messages for future sanitized logging, but those messages must not be persisted in lifecycle payloads or returned to React in this refactor.

Provision steps:

```rust
pub enum ProvisionedRemoteProvisionStep {
    CreateVolume,
    StartProvisioner,
    PollProvisioner,
    TerminateProvisioner,
    CreateEndpoint,
}
```

Cleanup steps:

```rust
pub enum ProvisionedRemoteCleanupStep {
    DeleteEndpoint,
    TerminateProvisioner,
    DeleteVolume,
}
```

Delete steps:

```rust
pub enum ProvisionedRemoteDeleteStep {
    DeleteEndpoint,
    TerminateProvisioner,
    DeleteVolume,
    DeleteLocalWorkspace,
}
```

Do not store or return a separate progress percent. Clients derive display progress from lifecycle operation payload and the operation-specific current step.

There is no `Cancelling` state in the new model.

## Service Responsibilities

`ProvisionedRemoteService` is the behavior owner for the provisioned remote runtime. It coordinates:

- workspace repository for workspace aggregate persistence
- lifecycle operation journal repository
- workflow catalog
- provider registry
- provider flow and cleanup helpers
- background lifecycle operation registry keyed by lifecycle operation ID
- runtime-agnostic event sink for domain workspace updates

Commands remain adapters:

- accept typed request DTOs
- call the service
- map returned domain state to binding-safe responses
- map service errors to `NativeCommandError`
- map domain workspace events to frontend events

Provider adapters remain under `provisioned_remote/providers/runpod/` and return only UI-safe snapshots and errors.

## Create Workspace Flow

`create_workspace(request)` creates the workspace aggregate. The workspace repository writes the generic workspace row with provisioned remote runtime JSON.

Flow:

```text
1. Resolve the workflow preset from the workflow catalog.
2. Build the initial Workspace state=NotProvisioned and ProvisionedRemoteRuntime with empty resources.
3. Insert workspace aggregate through the workspace repository in one atomic write:
   - id
   - runtime_type=provisioned_remote
   - provider_id from remote placement
   - state=not_provisioned
   - workflow_preset_json
   - runtime_json
   - created_at
   - updated_at
4. Return the assembled Workspace.
```

The provisioned remote service builds the initial workspace state and runtime domain data, then saves the workspace aggregate through the `workspace_catalog` workspace repository. The repository owns writing the workspace row, including derived provider_id, state, and runtime_json, as one atomic aggregate write.

## Provision Flow

`provision_workspace(workspace_id)` starts provisioning in a native background lifecycle operation runner and returns after the lifecycle operation is durably created.

Flow:

```text
1. Start transaction.
2. Load workspace row and provisioned remote runtime JSON.
3. Query lifecycle_operations for this workspace_id where state=running.
4. Reject only if a running lifecycle operation exists for this workspace.
5. Create lifecycle operation:
   - state=running
   - payload_json=Provision { step=None }
6. Keep workspace state unchanged; in-flight provisioning is represented by the lifecycle operation.
7. Commit transaction.
8. Assemble ProvisionWorkspaceResponse with the unchanged Workspace and created lifecycle operation.
9. Register and spawn a background lifecycle operation runner keyed by operation_id.
10. Return ProvisionWorkspaceResponse.
11. Execute lifecycle steps in the background runner:
   - CreateVolume
   - StartProvisioner
   - PollProvisioner
   - TerminateProvisioner
   - CreateEndpoint
12. Before each provider call or poll, set the current provision step in payload_json.
13. After provider success or failure, transactionally update lifecycle operation payload_json and state.
14. When provider progress changes runtime resources, transactionally update workspace runtime_json and workspaces.updated_at.
15. Emit a best-effort lifecycle operation event after each committed lifecycle operation update.
16. On success, transactionally set workspace state=Ready, mark lifecycle operation completed, and update workspaces.updated_at.
17. On failure, transactionally preserve resource snapshots, set the provision payload error to the mapped `ProvisionedRemoteLifecycleError`, mark lifecycle operation failed, and update workspaces.updated_at. Set workspace state to `CleanupRequired { reason: ProvisionFailed }` when any remote resource snapshot remains. If no remote resource snapshot remains, set workspace state to `Invalid { reason: ProvisionFailed }`.
18. After each committed workspace state/resource update, emit a best-effort domain workspace event through the service event sink.
```

Provider calls happen outside database transactions.

`TerminateProvisioner` remains part of the provision lifecycle operation because the provisioner pod is transient and must be cleaned up after provisioning succeeds or fails.

The background lifecycle operation registry is in-memory. It prevents duplicate in-process execution for a lifecycle operation, but durable correctness comes from the lifecycle operation journal, the unique running operation constraint, and startup stale handling.

## Cleanup Flow

`cleanup_workspace(workspace_id)` starts cleanup in a native background lifecycle operation runner and returns after the lifecycle operation is durably created.

Flow:

```text
1. Start transaction.
2. Load workspace row and provisioned remote runtime JSON.
3. Query lifecycle_operations for this workspace_id where state=running.
4. Reject only if a running lifecycle operation exists for this workspace.
5. Create lifecycle operation:
   - state=running
   - payload_json=Cleanup { step=None }
6. Keep workspace state unchanged; in-flight cleanup is represented by the lifecycle operation.
7. Commit transaction.
8. Assemble CleanupWorkspaceResponse with the unchanged Workspace and created lifecycle operation.
9. Register and spawn a background lifecycle operation runner keyed by operation_id.
10. Return CleanupWorkspaceResponse.
11. Execute cleanup lifecycle steps in the background runner:
   - DeleteEndpoint if endpoint exists.
   - TerminateProvisioner if provisioner exists.
   - DeleteVolume if volume exists.
12. Before each provider call, set the current cleanup step in payload_json.
13. Treat expected not-found provider errors as successful cleanup for that resource.
14. On success, transactionally clear resources, set workspace state=NotProvisioned, mark lifecycle operation completed, and update workspaces.updated_at.
15. On failure, transactionally preserve remaining resource snapshots, set the cleanup payload error to the mapped `ProvisionedRemoteLifecycleError`, mark lifecycle operation failed, and update workspaces.updated_at. Set workspace state to `CleanupRequired { reason: CleanupFailed }` when any remote resource snapshot remains. If no remote resource snapshot remains, set workspace state to `Invalid { reason: CleanupFailed }`.
16. Emit lifecycle operation and workspace events after committed updates.
```

## Startup Stale Handling

During app bootstrap, after database connection and schema bootstrap:

```text
1. Query lifecycle_operations where state=running.
2. For each running lifecycle operation:
   - set the runtime-specific payload error to `AppInterrupted`
   - mark lifecycle operation state=Stale
   - load the related workspace row and provisioned remote runtime JSON
   - set workspace state to `CleanupRequired { reason: OperationInterrupted }` if resource snapshots remain
   - otherwise set workspace state to `Invalid { reason: OperationInterrupted }`
   - update workspaces.updated_at
3. Commit updates transactionally per affected workspace/lifecycle operation.
```

No provider calls happen during startup.

## Event Boundary

Provisioned remote services must not depend on Tauri runtime APIs.

The service receives an injected event sink that accepts domain events, for example:

```rust
pub enum ProvisionedRemoteEvent {
    LifecycleOperationChanged {
        workspace_id: String,
        operation_id: LifecycleOperationId,
        operation: LifecycleOperation,
    },
    WorkspaceChanged {
        workspace_id: String,
        workspace: Workspace,
    },
    WorkspaceDeleted { workspace_id: String },
}
```

The service emits events only after the corresponding runtime or lifecycle journal state has been durably committed. Event delivery is best effort. Durable SQLite state remains the source of truth if an event is missed.

The Tauri adapter maps domain events to frontend events. Frontend event payloads use binding-safe response DTOs, for example `WorkspaceResponse` and `LifecycleOperationResponse`, but those response types must not leak into the provisioned remote service.

Frontend-facing native contract:

```text
1. provision_workspace(workspace_id) returns ProvisionWorkspaceResponse with the Workspace and created lifecycle operation.
2. LifecycleOperationChanged events are emitted globally across all workspaces through typed Tauri subscriptions.
3. WorkspaceChanged and WorkspaceDeleted events are emitted for stable workspace state/resource changes.
4. get_running_lifecycle_operations returns in-flight operations for reload or missed-event recovery.
5. get_latest_lifecycle_operation(workspace_id) returns the latest lifecycle operation for one workspace, including failed or stale operation payload error detail after reload.
6. No progress command is exposed to advance provider work.
```

Implementing or redesigning frontend behavior is out of scope for this refactor.

## Subscription Boundary

Frontend subscriptions are Tauri adapter contracts, not provisioned remote service contracts.

The service continues to emit runtime-agnostic domain events through the injected event sink. The Tauri adapter maps those domain events to binding-safe subscription DTOs and exposes them with `tauri-specta` typed events.

Required frontend subscription DTOs:

```rust
#[derive(Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct LifecycleOperationChangedEvent {
    pub workspace_id: String,
    pub operation_id: String,
    pub operation: LifecycleOperationResponse,
}

#[derive(Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct WorkspaceChangedEvent {
    pub workspace_id: String,
    pub workspace: WorkspaceResponse,
}

#[derive(Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct WorkspaceDeletedEvent {
    pub workspace_id: String,
}
```

Register these events in the Tauri Specta builder with `collect_events![...]` alongside the existing command registration. Keep `builder.mount_events(app)` in setup so generated frontend bindings expose typed event helpers.

Subscription DTOs must use frontend-safe response types. They must not expose domain entities directly, raw provider responses, worker payloads, credentials, credential-bearing URLs, SDK debug output, stack traces, or arbitrary messages.

The frontend subscribes to global lifecycle and workspace events and filters by `workspace_id` when a view needs one workspace. Subscriptions are notification hints only. Durable SQLite state remains authoritative, and reload or missed-event recovery must use `get_running_lifecycle_operations` and `get_latest_lifecycle_operation(workspace_id)`.

## Background Execution And Cancellation

The new model removes `cancel_workspace_provisioning` from the current command surface.

Mid-flight cancellation is deferred. If a user needs to stop after a failed, stale, or partial provision, the app should call `cleanup_workspace`.

Future cancellation should build on the background executor by adding:

- cancellation tokens
- explicit cancellation checkpoints between provider calls and poll cycles
- command API for canceling an active lifecycle operation
- cleanup or cleanup-required transitions after cancellation

The lifecycle operation journal stores durable lifecycle operation state independently of the in-memory background lifecycle operation registry.

## Delete Workspace Flow

`delete_workspace(workspace_id)` removes remote resources and then removes the local workspace records.

This differs from `cleanup_workspace(workspace_id)`: cleanup leaves the workspace in the catalog with state `NotProvisioned`, while delete removes the workspace from the catalog after cleanup succeeds.

Flow:

```text
1. Start transaction.
2. Load workspace row and provisioned remote runtime JSON.
3. Query lifecycle_operations for this workspace_id where state=running.
4. Reject only if a running lifecycle operation exists for this workspace.
5. If no resource snapshots remain:
   - create lifecycle operation:
     - state=running
     - payload_json=Delete { step=None }
   - mark lifecycle operation completed
   - assemble DeleteWorkspaceResponse with the completed lifecycle operation
   - delete lifecycle operation journal rows for the workspace
   - delete the workspaces row
   - commit transaction
   - emit WorkspaceDeleted
   - return DeleteWorkspaceResponse
6. If resource snapshots remain, create lifecycle operation:
   - state=running
   - payload_json=Delete { step=None }
7. Keep workspace state unchanged; in-flight delete cleanup is represented by the lifecycle operation.
8. Commit transaction.
9. Assemble DeleteWorkspaceResponse with the unchanged Workspace and created lifecycle operation.
10. Register and spawn a background lifecycle operation runner keyed by operation_id.
11. Return DeleteWorkspaceResponse.
12. Execute cleanup lifecycle steps:
   - DeleteEndpoint if endpoint exists.
   - TerminateProvisioner if provisioner exists.
   - DeleteVolume if volume exists.
13. Before each provider call, set the current delete step in payload_json.
14. Treat expected not-found provider errors as successful cleanup for that resource.
15. On cleanup success, transactionally:
   - mark lifecycle operation completed
   - delete lifecycle operation journal rows for the workspace
   - delete the workspaces row
16. Emit WorkspaceDeleted.
17. On cleanup failure, preserve remaining resource snapshots, set the delete payload error to the mapped `ProvisionedRemoteLifecycleError`, mark lifecycle operation failed, update workspaces.updated_at, and set workspace state to `CleanupRequired { reason: DeleteFailed }` when any remote resource snapshot remains. If no remote resource snapshot remains, set workspace state to `Invalid { reason: DeleteFailed }`.
18. Emit LifecycleOperationChanged and WorkspaceChanged with the failed cleanup state.
```

Provider calls happen outside database transactions.

Delete removes the lifecycle operation journal rows for the workspace after the delete operation is completed. The journal exists for native lifecycle correctness and progress rendering, not as a long-term history feature, and deleted workspaces do not need retained lifecycle operation history in this pre-v1 refactor.

## Error Handling

The refactor uses three error surfaces:

1. Generic persisted lifecycle state for recovery decisions.
2. Runtime-specific lifecycle payload errors for UI-safe failure detail.
3. Command errors for synchronous failures before an operation is durably created.

Internal native errors are implementation details. Repository, provider adapter, service orchestration, and Tauri adapter errors must never leak directly to React, generated frontend types, workspace JSON, lifecycle journal rows, events, or logs.

### Generic Lifecycle State

Workspace and lifecycle operation states stay coarse and stable:

```rust
pub enum WorkspaceState {
    NotProvisioned,
    Ready,
    CleanupRequired { reason: WorkspaceCleanupRequiredReason },
    Invalid { reason: WorkspaceRuntimeInvalidReason },
}

pub enum WorkspaceCleanupRequiredReason {
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    OperationInterrupted,
}

pub enum WorkspaceRuntimeInvalidReason {
    OperationInterrupted,
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    CorruptRuntimeState,
}

pub enum LifecycleOperationState {
    Running,
    Completed,
    Failed,
    Stale,
}
```

These types record recovery semantics only. They must not contain provider status codes, worker-specific failure variants, arbitrary messages, retryability flags, stack traces, raw provider payloads, worker context, credentials, credential-bearing URLs, SDK debug output, or environment data.

### Runtime Payload Errors

Concrete provider and provisioner failure detail belongs in the runtime-specific lifecycle operation payload, not in workspace state and not in `LifecycleOperationState`.

```rust
pub enum ProvisionedRemoteLifecycleOperationPayload {
    Provision {
        step: Option<ProvisionedRemoteProvisionStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Cleanup {
        step: Option<ProvisionedRemoteCleanupStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
    Delete {
        step: Option<ProvisionedRemoteDeleteStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    },
}

pub enum ProvisionedRemoteLifecycleError {
    AppInterrupted,
    ProviderAdapterUnavailable,
    ProviderSecretUnavailable,
    ProviderApiFailed { reason: ProviderApiError },
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
    InvalidRuntimeState,
}

pub enum ProviderApiError {
    Unauthorized,
    InsufficientPermissions,
    RateLimited,
    Timeout,
    RequestFailed,
}
```

`ProviderAdapterUnavailable` means native could not resolve or use the configured provider adapter. `ProviderApiFailed { reason }` means a provider API request was made and failed. `ProviderSecretUnavailable` means the required local credential is missing or unavailable.

`ProviderApiError::RequestFailed` is intentionally message-free in persisted state and frontend responses. Provider adapters may keep internal diagnostic messages for a future sanitized logging pipeline, but those messages must not be persisted or returned to React in this refactor.

`ProvisionerUnavailable` and `ProvisionerResponseInvalid` are native client-side failures while talking to the provisioner worker. `ProvisionerFailed` means the worker reported a terminal failure. Do not persist provisioner error codes, messages, or context in this refactor; worker failure codes are future sanitized logging/diagnostics material.

### Operation Failure Flow

Before each provider call or worker poll, the lifecycle runner updates the operation payload step. If the call fails after the lifecycle operation has been durably created:

1. Map the internal error to `ProvisionedRemoteLifecycleError`.
2. Store that error in the runtime-specific lifecycle payload.
3. Mark the lifecycle operation `Failed` or `Stale`.
4. Update workspace state to `CleanupRequired` or `Invalid` using the coarse workspace reason.
5. Emit lifecycle operation and workspace events after commit.

Startup stale handling sets the payload error to `AppInterrupted`, marks the lifecycle operation `Stale`, and then updates workspace state based on whether resource snapshots remain.

### Command Errors

`NativeCommandError` remains the only frontend command error envelope and carries only a stable code:

```rust
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
}
```

Do not add a generic error message or context object. User-facing text should be derived by the frontend from stable codes and local UI copy.

Command errors are for synchronous failures before a lifecycle operation is durably created or scheduled. After `provision_workspace`, `cleanup_workspace`, or `delete_workspace` has returned a lifecycle operation, provider/provisioner/runtime failures must be reported through lifecycle operation state, runtime payload error, workspace state, and events.

Command codes should be stable and behavior-oriented:

```text
LifecycleOperationAlreadyRunning
ProviderAdapterUnavailable
ProviderSecretUnavailable
ProviderUnauthorized
ProviderInsufficientPermissions
ProviderRateLimited
ProviderTimeout
ProviderRequestFailed
ProvisionerUnavailable
ProvisionerResponseInvalid
ProvisionerFailed
InvalidWorkspaceState
WorkspaceStorageUnavailable
WorkspaceStorageQueryFailed
WorkspaceStorageCorrupt
WorkspaceStorageSchemaMismatch
WorkspaceAlreadyExists
WorkspaceNotFound
WorkflowCatalogInvalid
```

Rename current command codes as part of the refactor:

```text
ProvisioningAlreadyRunning -> LifecycleOperationAlreadyRunning
InvalidProvisioningState -> InvalidWorkspaceState
ProviderUnavailable -> ProviderAdapterUnavailable
```

Do not add command codes for every workspace failure reason. `ProvisionedRemoteProvisionFailed`, `ProvisionedRemoteCleanupFailed`, and `ProvisionedRemoteCleanupRequired` are not needed as command codes for background operation failures because those failures occur after the lifecycle operation exists and should be visible through the lifecycle payload and workspace state.

### Subscription Errors

Subscription failures are not lifecycle failures and are not domain errors.

If the frontend fails to attach a listener, loses a listener, or misses an event, workspace and lifecycle state must not change. Recovery is handled by reading durable state through `get_running_lifecycle_operations` and `get_latest_lifecycle_operation(workspace_id)`.

Backend event emission happens after the durable commit and is best effort. A Tauri emit failure must not roll back the committed workspace or lifecycle operation update, must not mark the lifecycle operation `Failed` or `Stale`, and must not be converted into `NativeCommandError`.

Typed event mapping or serialization failures are Tauri adapter implementation errors. They must not leak raw internal errors to React and must not introduce a subscription error DTO in this refactor. Future sanitized logging can record these diagnostics, but logging is out of scope here.

### Removed Legacy Errors

Remove from the persisted domain model:

```text
ProvisionedRemoteComputeProvisioningError
ProvisionedRemoteComputeProvisioningState
ProvisionedRemoteComputeProvisioningStatus
ProvisionedRemoteComputeProvisioningPhase
ProvisionedRemoteComputeProvisioningState.percent
CancellationCleanupFailed
InvalidProvisioningState { message }
```

`CancellationCleanupFailed` is removed because cancellation is out of scope. `InvalidProvisioningState { message }` is replaced by stable internal errors, command codes, and coarse persisted state; arbitrary messages do not belong in the domain snapshot.

Remove or defer from the provisioned remote service error surface in this refactor:

```text
ExecuteWorkspaceNotReady
ExecuteWorkspaceMissingEndpoint
ExecuteWorkspaceNotImplemented
DeleteWorkspaceFailed { message }
```

`execute_workspace` behavior is not part of this lifecycle refactor. Delete failures should be represented through the delete lifecycle operation and coarse workspace state instead of a single message-bearing service error.

### Failure Detail Recovery

Concrete failed or stale operation detail must be recoverable after reload. `get_latest_lifecycle_operation(workspace_id)` returns the latest lifecycle operation for one workspace, including runtime payload error detail. This is not a general history feature; it exists to recover the latest failed or stale operation state for UI display.

## Command Boundary

Target workspace commands:

```text
create_workspace
provision_workspace
cleanup_workspace
delete_workspace
get_workspace
get_workspace_catalog
get_running_lifecycle_operations
get_latest_lifecycle_operation
```

Remove or defer:

```text
cancel_workspace_provisioning
```

Responses should use the renamed runtime language:

```text
WorkspaceRuntimeResponse::ProvisionedRemote(...)
WorkspaceStateResponse
LifecycleOperationResponse
```

Command responses:

```rust
pub struct ProvisionWorkspaceResponse {
    pub workspace: WorkspaceResponse,
    pub lifecycle_operation: LifecycleOperationResponse,
}

pub struct CleanupWorkspaceResponse {
    pub workspace: WorkspaceResponse,
    pub lifecycle_operation: LifecycleOperationResponse,
}

pub struct DeleteWorkspaceResponse {
    pub workspace_id: String,
    pub lifecycle_operation: LifecycleOperationResponse,
}

pub struct RunningLifecycleOperationsResponse {
    pub operations: Vec<LifecycleOperationResponse>,
}

pub struct LatestLifecycleOperationResponse {
    pub operation: Option<LifecycleOperationResponse>,
}
```

Command and event DTOs live at the Tauri adapter boundary. `ProvisionedRemoteService` returns and emits domain entities only; it must not depend on generated binding DTOs or Tauri runtime types.

Do not add standalone lifecycle operation history commands in this refactor. `get_running_lifecycle_operations` exists for frontend hydration and missed-event recovery only. `get_latest_lifecycle_operation(workspace_id)` exists so the frontend can recover the latest failed or stale operation payload error after reload without introducing a general history surface.

## Testing

Add or update native tests for:

- provisioned remote domain state transitions
- lifecycle operation creation
- duplicate running-lifecycle-operation rejection
- provision_workspace returning a workspace and created provision lifecycle operation
- background provision lifecycle step sequencing
- background provision success marking workspace state Ready and lifecycle operation Completed
- cleanup step sequencing
- delete workspace removes remote resources before deleting local workspace records
- delete workspace without resource snapshots creates and completes a delete lifecycle operation
- delete workspace removes lifecycle operation journal rows after completion
- provider failure marking lifecycle step and lifecycle operation failed
- provider request failure stores message-free `ProviderApiFailed { reason: RequestFailed }`
- provisioner-reported failure stores `ProvisionerFailed` without persisting worker error code, message, or context
- malformed or missing provisioner failure payload maps to `ProvisionerResponseInvalid`
- background provider failure emitting lifecycle operation and workspace events while preserving durable failed state
- latest lifecycle operation query returns failed or stale payload error detail after reload
- delete workspace cleanup failure preserves the workspace and emits WorkspaceChanged
- delete workspace success emits WorkspaceDeleted
- workspace invalid or cleanup-required state
- preserving resources after cleanup failure
- treating expected not-found cleanup errors as success
- startup stale handling
- no provider calls during startup stale handling
- SQLite schema bootstrap for workspace and lifecycle operations
- create workspace writes provider_id, state, and runtime_json in one workspace row
- workspace row plus provisioned remote runtime JSON round trips
- provisioned remote runtime provider_id round trips without parsing runtime_json
- workspace state round trips through normalized workspaces.state
- missing or invalid runtime JSON treated as corrupt or schema invalid
- running lifecycle operation query without JSON parsing
- operation-specific current step in payload_json
- lifecycle operation kind is stored in payload_json, not as a normalized column
- transactional workspace/lifecycle operation updates
- runtime-agnostic event sink receives domain lifecycle operation and workspace events after committed updates
- Tauri adapter maps domain events to `tauri-specta` typed subscription DTOs
- generated bindings expose typed lifecycle and workspace event subscriptions
- subscription emit failures do not change durable workspace or lifecycle operation state
- mutating commands reject only while a running lifecycle operation exists for the same workspace
- delete_workspace rejects only while a running lifecycle operation exists for the same workspace
- command error mappings to granular stable codes
- generated bindings reflect `ProvisionedRemote` naming, typed subscriptions, `provision_workspace`, `delete_workspace`, `get_workspace`, `get_running_lifecycle_operations`, `get_latest_lifecycle_operation`, and no cancellation command

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

- Workspace persistence stores stable workspace state in the workspace row. Runtime-specific modules are internal workspace repository mapping helpers, not a public generic runtime persistence registry.
- Keep provider trait redesign narrow. Rename existing traits to `ProvisionedRemote*`, but do not generalize for a second provider.
- Existing pre-v1 workspace database contents can be discarded rather than migrated.
- Service-owned transactions should coordinate workspace and lifecycle journal consistency. Repositories should support transaction-aware methods where atomic multi-table updates are required.
- Do not add `last_observed_at` or similar observation timestamps until an explicit provider observation or reconciliation flow exists.
