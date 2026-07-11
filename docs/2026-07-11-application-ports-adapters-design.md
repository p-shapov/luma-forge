# Application Ports and Adapters Design

**Date:** 2026-07-11

## Scope

This document defines the future application, port, adapter, and persistence boundaries for the active `src-tauri/src` refactor. It uses the current raw primitives in `src-tauri/src/infra` and the catalog shape under `new_bundled/catalog/entries` as the source context.

The design covers:

- provider-neutral workspaces;
- provider-specific runtime workflows, initially RunPod;
- lifecycle operation journaling;
- workflow and runtime catalog access;
- secret storage and identity validation;
- application-facing adapters over raw infra primitives.

The design does not include Tauri commands, facade DTOs, Specta/codegen, operation reconciliation or resume, multiple simultaneous runtimes per workspace, adapter tests, or infra tests.

## Implementation Baseline

`src-tauri/src` is the only active and compilable backend source tree. The earlier `new_src` example tree is removed before implementation and is not an implementation target. Any ignored `old_src` copy is optional local reference material only: it is never imported, compiled, committed, or used as a compatibility fallback. Git history remains the canonical source for removed code.

The source-tree cutover is already complete and must not appear in the implementation plan. Before application work begins, the active crate retains a minimal `src-tauri/src/lib.rs` that declares `pub mod infra;`, and `src-tauri/Cargo.toml` contains no path dependency on files that exist only in ignored `old_src`.

## Dependency Direction

The three layers have distinct responsibilities:

```text
application <- adapters -> infra
```

- `application` owns use cases, state transition rules, application models, ports, and typed application errors.
- `infra` owns raw primitives: bundled catalog readers, HTTP clients and generated transport DTOs, SeaORM entities and database connections, and account-based keyring access. It never imports `application`.
- `adapters` implement application ports using infra primitives. They map models and errors in both directions and may compose multiple raw primitives.

Tauri will be a separate inbound adapter in a later scope. Tauri and Specta types or derives must not appear in `application`.

## Proposed Module Shape

```text
application/
  workspace/
    ports/
  runtimes/
    runpod/
  lifecycle/
    progress/
    ports/
  secrets/
    ports/

adapters/
  bundled/
    workflow_catalog.rs
    runpod_runtime_catalog.rs
  runpod/
    runtime_provider.rs
    identity_provider.rs
  hugging_face/
    identity_provider.rs
  sqlite/
    workspace_repository.rs
    runpod_runtime_repository.rs
    lifecycle_operation_repository.rs
  keyring/
    secret_store.rs

infra/
  bundled/
  clients/
  keyring/
  sqlite/
```

Adapters are grouped by the external mechanism they translate, not by generic stereotypes such as `providers`, `catalogs`, `repositories`, and `storages`. Port files use Rust `snake_case` names.

## Workspace

### Model

A workspace is provider-neutral and immutable after creation. Its durable fields are:

- workspace ID;
- workflow ID;
- workflow revision;
- creation timestamp.

The selected workflow cannot change after workspace creation.

`state` and `runtime_kind` are not stored on the workspace row. A workspace without an attached runtime projects `WorkspaceStatus::NotProvisioned`. When a runtime exists, the workspace status is mapped from the provider runtime status.

The workspace application read model may include `attached_runtime: Option<RuntimeKind>`. This value comes from the runtime anchor relation; it is not a column owned by the workspace table.

Application outputs must not expose the persistence split between workspace and runtime. A future inbound adapter may map the application result into a client-safe workspace snapshot with a typed provider-specific runtime summary.

### Repository Port

`WorkspaceRepository` owns durable workspace operations:

- create an immutable workspace;
- get one workspace with its optional attached runtime kind;
- list workspaces with their optional attached runtime kinds;
- delete a workspace.

Workspace deletion is allowed only when no runtime is attached and no lifecycle operation is `Running`. Lifecycle journal entries are retained after workspace deletion.

## Runtime Identity and Persistence

Only one runtime may be attached to a workspace across all providers.

SQLite enforces this through a provider-neutral anchor and a provider-specific extension:

```text
workspaces
  id PK

workspace_runtimes
  workspace_id PK/FK -> workspaces.id
  provider_kind

runpod_workspace_runtimes
  workspace_id PK/FK -> workspace_runtimes.workspace_id
  state
  datacenter_id
  gpu_id
  volume_size_gb
  network_volume_id
  provisioner_pod_id
  template_id
  endpoint_id
```

The anchor prevents two provider-specific runtime rows from being created concurrently for one workspace. Adding another provider adds another typed extension table rather than nullable provider columns or opaque JSON.

The provider is chosen for each `Provision`. After successful cleanup, the provider anchor and provider-specific runtime row are deleted, so the workspace returns to `NotProvisioned` and retains no provider choice.

Multiple simultaneous runtimes per workspace are intentionally out of scope. Supporting them would require runtime identity, lifecycle targeting, concurrency, workspace projection, and workflow execution semantics to change together.

## RunPod Runtime

### State Machine

The RunPod runtime states and transitions are:

```text
Provisioning -> Ready
Provisioning -> Failed
Ready        -> CleaningUp
Failed       -> CleaningUp
CleaningUp   -> runtime deleted
CleaningUp   -> Failed
```

The chosen `datacenter_id`, `gpu_id`, and `volume_size_gb` are immutable for the lifetime of the runtime. State and provider resource IDs change during lifecycle workflows.

Calling `Provision` produces typed application errors by current state:

- `Ready` -> `AlreadyProvisioned`;
- `Failed` -> `RuntimeFailed`;
- `Provisioning` or `CleaningUp` -> `OperationInProgress`.

Calling `Cleanup` when no runtime exists returns an explicit `NotProvisioned` error. Cleanup is allowed from `Ready` and `Failed`.

### Workflow Ownership

The RunPod-specific `Provision` and `Cleanup` sequences live in `application/runtimes/runpod`. They own RunPod runtime transitions and call lifecycle journaling collaborators. The provider-neutral lifecycle module does not know RunPod API call order.

Provision steps:

```text
CreateNetworkVolume
-> StartProvisionerPod
-> PollProvisioner
-> TerminateProvisionerPod
-> CreateTemplate
-> CreateEndpoint
```

Cleanup steps:

```text
DeleteEndpoint
-> DeleteTemplate
-> TerminateProvisionerPod
-> DeleteNetworkVolume
```

Cleanup provider calls are idempotent: an already absent provider resource counts as a successful step. Calling Cleanup without an attached runtime remains an application error.

### Provider Port

`RunpodRuntimeProvider` is one cohesive port containing only the provider operations required by Provision and Cleanup. It is not split into volume, pod, template, and endpoint traits because those operations share one provider, credential, client, and failure boundary.

The port uses application RunPod request/result types. Generated GraphQL and REST DTOs remain in infra and are mapped by `adapters/runpod/runtime_provider`.

The RunPod runtime service reads `SecretKind::RunpodApiKey` through `SecretStore` and passes the transient `SecretString` to `RunpodRuntimeProvider`. The account name used by the keyring never leaves the keyring adapter.

### Repository Port

`RunpodRuntimeRepository` loads the provider runtime and persists runtime/lifecycle transitions.

Its atomic `save_transition(runtime, operation)` consistency boundary writes:

- the runtime anchor when starting Provision;
- the RunPod runtime state, immutable configuration, and known resource IDs;
- the provider-neutral lifecycle operation;
- the RunPod-specific lifecycle progress.

After each successful provider call, `save_transition` stores the new resource ID and advances the lifecycle step in one SQLite transaction. On failure it atomically marks both runtime and lifecycle operation `Failed`. On successful Cleanup it deletes the runtime extension and anchor while marking the lifecycle operation `Succeeded`.

The separate runtime and lifecycle state machines remain separate application models. Sharing a persistence transaction does not merge them into one state machine.

## Lifecycle Operations

### Purpose

Lifecycle operations are a durable journal for UI progress and diagnostics. Reconciliation and operation resume are not in scope.

Every lifecycle operation stores:

- operation ID;
- historical workspace ID;
- operation kind;
- operation state;
- trace ID;
- created, updated, and finished timestamps;
- provider-specific progress.

Operation kinds are `Provision` and `Cleanup`. Operation states are:

```text
Running -> Succeeded
Running -> Failed
```

Only one `Running` lifecycle operation may exist for a workspace. Persistence must enforce this invariant atomically.

Every operation receives a `trace_id` at creation. Error codes and error messages are not stored in the journal. A failed operation retains its state, current step, and trace ID; technical details are read from local logs using that trace ID.

### Provider-Specific Progress

Progress is a component of the lifecycle operation aggregate, not an independent aggregate and not an external provider request payload.

```text
LifecycleProgress::Runpod(
  RunpodProgress::Provision(RunpodProvisionStep)
  | RunpodProgress::Cleanup(RunpodCleanupStep)
)
```

Separate Provision and Cleanup step enums make invalid kind/step combinations unrepresentable. The shared `TerminateProvisionerPod` name appears in both step enums.

The current step is persisted before the corresponding provider call. If the call fails, that step remains recorded as the failure location. On success, the operation retains its final step.

The physical schema may use two tables:

```text
lifecycle_operations
  provider-neutral operation fields

runpod_lifecycle_progress
  operation_id PK/FK -> lifecycle_operations.id
  step
```

These tables are written through one logical runtime transition boundary. There is no separate `RunpodLifecyclePayloadsRepository` port.

`lifecycle_operations.workspace_id` is a historical identifier, not cascade-owned workspace data. Deleting a workspace does not delete its journal.

### Journal Port

`LifecycleOperationRepository` supports provider-neutral journal reads and startup discovery:

- `recent(limit)`, ordered by `created_at DESC`;
- `recent_for_workspace(workspace_id, limit)`, ordered by `created_at DESC`;
- discovery of `Running` operations for startup recovery and workspace guards.

Cursor pagination is deferred. A bounded recent query is sufficient for the current local pre-v1 journal.

### Interrupted Operations

On application startup, any remaining `Running` lifecycle operation and its corresponding provider runtime are atomically changed to `Failed`, retaining the current step and trace ID. This is minimal local crash recovery, not provider reconciliation. It prevents an interrupted process from permanently blocking the one-running-operation invariant.

## Catalogs

Repositories do not read bundled catalogs. Catalog and repository ports are independent application dependencies coordinated by application use cases.

### Workflow Catalog

`WorkflowCatalog` replaces the ambiguous name `WorkspaceCatalog`.

```text
list_summaries() -> Vec<WorkflowSummary>
get(id, revision) -> Option<WorkflowDefinition>
```

`WorkflowSummary` contains only:

- ID;
- revision;
- name;
- description.

Summaries form a flat list with deterministic ordering by ID and revision. Pagination is unnecessary for the bundled catalog. The application contract stays lightweight even if the first adapter implementation reads a full bundled workflow entry before projecting its summary.

The current sample workflow metadata does not contain `description`. The current catalog contract and entries must add it when this port is implemented; the adapter must not invent a fallback value.

An unknown user-selected workflow key returns `None`. A broken reference inside an existing bundled workflow is an `InvalidCatalog` error.

### RunPod Runtime Catalog

Workflow contract requirements are provider-specific. A RunPod requirement contains typed references to its provisioner and endpoint runtime contracts. Runtime presets and runtime contract entries remain reusable catalog documents.

`RunpodRuntimeCatalog` resolves the references needed by the RunPod runtime workflow:

```text
resolve(runtime_preset_ref, runpod_contract_requirements)
  -> RunpodRuntimeDefinition

RunpodRuntimeDefinition
  runtime_preset
  provisioner_contract
  endpoint_contract
```

This keeps catalog reference traversal and missing-reference validation inside the bundled adapter. The RunPod runtime service receives a resolved application definition and does not know the bundled directory or document layout.

One bundled adapter may implement both `WorkflowCatalog` and `RunpodRuntimeCatalog` over the same raw `infra::bundled::Catalog`.

## Secrets

### Models and Ports

Current secret kinds are:

- `RunpodApiKey`;
- `HuggingFaceApiKey`.

Identity is a common client-safe application model:

```text
Identity
  key_name: Option<String>
  username: Option<String>
  email: Option<String>
```

Providers populate only fields available from their identity APIs. Missing optional identity fields are not errors.

Both RunPod and Hugging Face adapters implement the same port:

```text
SecretIdentityProvider
  identity(&SecretString) -> Identity
```

`SecretStore` owns typed secret persistence operations:

```text
exists(SecretKind)
get(SecretKind) -> Option<SecretString>
insert(SecretKind, SecretString)  # explicit AlreadyExists
delete(SecretKind)               # explicit NotFound
```

The keyring adapter exclusively owns the mapping from `SecretKind` to keyring account names. Account names are not application models and are not duplicated in provider adapters.

### Behavior

Setting a secret:

1. reject the request if the kind is already configured;
2. validate the candidate credential through the selected `SecretIdentityProvider`;
3. insert it only after successful validation;
4. return the safe Identity from that validation.

Secrets cannot be overwritten. Replacement requires explicit `delete` followed by `set`. Deleting an absent secret returns an explicit `NotConfigured` application error.

Secret status is `Missing` or `Configured` and performs no network call or raw secret read. An explicit identity query reads the stored credential and performs a live provider identity lookup. Identity is never persisted.

Raw credentials are transient `SecretString` values. They are never returned in application snapshots, logged, included in errors, persisted outside secure storage, or exposed to future Tauri/Specta contracts.

## Error Boundaries

There is no global `ApplicationError`. Each module owns a small typed error enum.

Expected application error categories are:

- workspace: `NotFound`, `AlreadyExists`, `RuntimeAttached`, `OperationRunning`;
- RunPod runtime: `AlreadyProvisioned`, `RuntimeFailed`, `OperationInProgress`, `NotProvisioned`, `CredentialMissing`, `ProviderUnavailable`;
- lifecycle: `OperationAlreadyRunning`, `InvalidTransition`;
- secrets: `AlreadyConfigured`, `NotConfigured`, `InvalidCredential`, `IdentityUnavailable`, `StorageUnavailable`;
- catalogs: user lookup absence through `Option`, and `InvalidCatalog` for broken internal references.

Adapters map `NetworkError`, `DbErr`, `KeyringStorageError`, `BundledCatalogError`, generated DTOs, and SeaORM entities into the corresponding application port contracts. Raw provider response bodies and infra error strings do not cross the adapter boundary.

## Behavioral Tests

Only application behavioral tests are in scope. They use fake ports and cover:

- RunPod Provision and Cleanup flow ordering;
- progress persisted before each provider call;
- successful and failed runtime/lifecycle transitions;
- cleanup tolerance for already absent provider resources;
- rejection of parallel `Running` operations;
- workspace deletion guards;
- workflow lookup and RunPod runtime definition orchestration;
- secret validate-before-insert behavior;
- prevention of secret overwrite;
- explicit failure when deleting a missing secret;
- startup conversion of interrupted `Running` operations and runtimes to `Failed`.

Adapter, infra, SQLite integration, Tauri, Specta, and generated contract tests are outside this scope.

## Deferred Work

The following work is explicitly deferred until a concrete requirement exists:

- provider reconciliation and lifecycle resume;
- cursor pagination for lifecycle history;
- multiple simultaneous runtimes per workspace;
- secret overwrite/update operations;
- a second compute provider implementation;
- Tauri commands, facade DTOs, Specta derives, and frontend codegen;
- adapter and infra integration tests.
