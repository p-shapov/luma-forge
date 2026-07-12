# Tauri Facade Design

**Date:** 2026-07-13

## Goal

Add the first active Tauri inbound facade and the `lib.rs` wiring point for the
current native backend. The facade exposes generated, UI-safe commands and
events while keeping Tauri, Specta, Serde transport derives, and frontend DTOs
out of `application/**`.

Every command is a diagnostics root. Application services remain authoritative
for validation, durable mutation, provider workflows, and background runtime
operations.

## Scope

This design covers:

- facade request, response, error, and event DTOs;
- Tauri command and event registration through Tauri Specta;
- workspace reads with a hydrated runtime;
- workflow and runtime-operation pagination;
- concrete facade and SQLite persistence dispatch, initially with one RunPod
  arm each;
- RunPod placement and provider identity queries;
- API-key setup and deletion;
- application-event mapping to Tauri events;
- fatal native bootstrap and dependency wiring in `lib.rs`;
- generated TypeScript bindings and focused verification.

This design does not include frontend feature work, workflow execution,
multiple runtimes per workspace, cancellation, retries, an event outbox,
runtime reconciliation, provider resource IDs in UI contracts, or a second
runtime provider.

## Core Decisions

- A workspace has at most one runtime: `Workspace::runtime` is
  `Option<Runtime>`.
- `Runtime` owns one provider-neutral `RuntimeState` and one tagged
  `RuntimeProvider`, initially `RuntimeProvider::Runpod`.
- `RuntimeState` is exactly `Provisioning`, `Ready`, `CleaningUp`, or `Failed`
  for every provider. Provider-internal states map into this shared lifecycle.
- `workspace_runtimes` remains the provider-neutral one-runtime anchor. It
  stores `runtime_kind` and the shared state and enforces cross-provider
  exclusivity.
- Provider extension tables reference `workspace_runtimes.workspace_id` with
  `ON DELETE CASCADE` and store only provider configuration and resources.
- The facade and SQLite persistence boundaries use concrete closed dispatchers
  with ordinary exhaustive enum matches. Dispatch arms route values only;
  provider behavior and persistence stay in provider-specific modules.
- There is no dispatcher trait, factory, dynamic registry, or plugin system.
- Application events already carry complete application snapshots. The Tauri
  sink maps them synchronously; there is no hydration relay or facade cache.
- Native initialization is all-or-nothing. A bootstrap failure aborts Tauri
  startup rather than installing a partial or failed managed state.

## Application Model And Persistence

### Workspace Owns The Runtime Relation

The application workspace becomes:

```rust
pub struct Workspace {
    pub id: String,
    pub workflow: CatalogRef,
    pub created_at: OffsetDateTime,
    pub runtime: Option<Runtime>,
}

pub struct Runtime {
    pub state: RuntimeState,
    pub provider: RuntimeProvider,
}

pub enum RuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

pub enum RuntimeProvider {
    Runpod(RunpodRuntime),
}
```

The provider runtime does not repeat `workspace_id`; its identity comes from
the parent workspace. `RuntimeProvider` determines `RuntimeKind`; the kind is
not duplicated inside `Runtime`. The RunPod variant owns only its placement
configuration and native-only resource IDs. Shared lifecycle state belongs to
`Runtime`.

`RuntimeOperation` stores `runtime_kind` explicitly. Operation history outlives
the runtime anchor after cleanup or workspace deletion, so operation and
progress hydration cannot infer the provider from the current workspace.

`WorkspaceService::create` generates the workspace UUID inside the application
boundary. `CreateWorkspaceRequest` therefore contains only the workflow
reference.

### Provider-Neutral SQLite Relation

The persisted shape is:

```text
workspaces
  id PK
  workflow_id
  workflow_revision
  created_at

workspace_runtimes
  workspace_id PK/FK -> workspaces.id ON DELETE CASCADE
  runtime_kind
  state

runpod_workspace_runtimes
  workspace_id PK/FK -> workspace_runtimes.workspace_id ON DELETE CASCADE
  datacenter_id
  gpu_id
  volume_size_gb
  provider resource IDs

runtime_operations
  id PK
  workspace_id
  runtime_kind
  operation_kind
  state
  trace and timestamps

runpod_runtime_operation_progress
  operation_id PK/FK -> runtime_operations.id
  step
```

This is a pre-v1 schema change; no migration, compatibility read, or fallback
is added. `workspace_runtimes` is the discriminator used for hydration and the
database guarantee that one workspace cannot own two provider extensions.

`WorkspaceRepository::get` and the paginated workspace read load the base row
and optional anchor first. `SqliteRuntimePersistenceDispatcher` matches the
stored kind and delegates provider extension hydration to the corresponding
provider persistence module. Page hydration groups selected workspace IDs by
kind and performs one batch read per present kind, not one read per workspace.

A provider extension without its anchor is prevented by the foreign key. An
anchor without its required provider extension is corrupt data. The reader
does not probe other provider tables or use a fallback.

### SQLite Persistence Dispatcher

The generic SQLite boundary has one closed
`SqliteRuntimePersistenceDispatcher`. It owns only exhaustive dispatch for
provider runtime snapshots and operation progress. RunPod entity access,
field mapping, state-independent provider data, and progress mapping live in
`adapters/sqlite/runpod_runtime_persistence.rs`.

Generic repositories may mention a provider only in an exhaustive dispatch
arm. They must not contain provider SQL, provider resource fields, provider
lifecycle branches, provider error interpretation, or provider DTO mapping.
Provider persistence functions receive `&DatabaseTransaction` so generic and
provider rows remain in one transaction without exposing SeaORM through an
application port.

### Runtime Transitions

`SqliteRuntimeTransitionRepository` owns the generic transaction. It validates
workspace identity and requires `Runtime.provider.kind()` to equal
`RuntimeOperation.runtime_kind` whenever the workspace contains a runtime. It
then writes the shared anchor/state and generic operation, dispatches the
provider snapshot and progress writes, and commits once.

Provision inserts the anchor, provider extension, operation, and progress.
Intermediate transitions update the shared state and provider extension.
Failed transitions and recovery set the shared state to `Failed` while
preserving provider resources. Successful cleanup persists the terminal
operation, deletes the anchor and therefore its provider extension by cascade,
and produces `Workspace { runtime: None }`. Operation history retains its
`runtime_kind` and provider progress after the anchor is gone.

A provider write failure rolls back anchor, operation, extension, and progress
writes together. Transition events remain `workspace_changed` followed by
`runtime_operation`, emitted only after commit.

## Facade Boundary

### Facade State

Tauri manages one owned `FacadeState` containing:

```text
FacadeState
├── WorkspaceService
├── SecretsService
├── RuntimeOperationQueryService
└── RuntimeDispatcher
    └── RunpodRuntimeService
```

Application services own their ports through `Arc`, matching the existing
RunPod runtime service and avoiding self-referential state. Commands do not
construct repositories, provider clients, or keyring adapters.

Pure catalog and journal reads still pass through application services. The
facade does not query SeaORM, bundled files, provider clients, or keyring
storage directly.

### Runtime Dispatcher

The facade dispatcher has one concrete RunPod service field and contains no
provider workflow behavior.

- `provision_workspace` matches the tagged runtime request and calls the
  corresponding provider service.
- `cleanup_workspace` loads the workspace once, matches its hydrated runtime,
  and calls the corresponding provider service.
- interrupted-operation recovery matches `RuntimeOperation.runtime_kind` and
  calls the corresponding provider recovery entry point.

Adding another provider adds one enum variant, one service field, and match
arms. Provider validation, credentials, lifecycle calls, and detached work stay
inside the provider service.

RunPod-specific code is permitted only in `application/runtimes/runpod/**`,
`adapters/runpod/**`, `adapters/sqlite/runpod_runtime_persistence.rs`, explicit
dispatch arms, composition-root wiring, and explicitly RunPod-specific facade
commands and DTOs.

### RunPod Placement

RunPod placement becomes an application query on the RunPod runtime boundary.
It reads the stored RunPod API key through `SecretStore`, calls the provider,
and returns normalized application placement models. The Tauri command never
reads a raw secret or calls `RunpodProvider` directly.

## Command Contracts

All facade inputs and outputs derive the required Serde and Specta types. Field
names use the generated TypeScript convention consistently.

| Command | Request | Response |
| --- | --- | --- |
| `get_workflows` | `{ offset, limit }` | `{ workflows, total }` |
| `get_workspaces` | `{ offset, limit }` | `{ workspaces, total }` |
| `create_workspace` | `{ workflow }` | `WorkspaceDto` |
| `delete_workspace` | `{ workspace_id }` | `()` |
| `provision_workspace` | `{ workspace_id, runtime }` | `{ workspace, operation }` |
| `cleanup_workspace` | `{ workspace_id }` | `{ workspace, operation }` |
| `get_runtime_operations` | `{ workspace_id?, offset, limit }` | `{ operations, total }` |
| `get_runpod_placement` | none | `RunpodPlacementDto` |
| `setup_runpod_api_key` | `{ api_key }` | `IdentityDto` |
| `setup_hugging_face_api_key` | `{ api_key }` | `IdentityDto` |
| `get_runpod_identity` | none | `IdentityDto` |
| `get_hugging_face_identity` | none | `IdentityDto` |
| `delete_runpod_api_key` | none | `()` |
| `delete_hugging_face_api_key` | none | `()` |

The provision input is a tagged enum:

```text
ProvisionRuntimeInput::Runpod {
  datacenter_id,
  gpu_id,
  volume_size_gb,
}
```

The provider selection is explicit in the provision request. Cleanup uses the
runtime already attached to the workspace.

No separate secret-status command is exposed. Missing credentials are reported
by the identity and provider commands through their typed errors.

## Pagination

The three list commands accept zero-based `offset` and `limit`. The facade
validates `limit` as `1..=100`; invalid values return the command-specific
`invalid_pagination` code.

Every response includes the total number of matching records before pagination.

- workflows use `id ASC, revision ASC` and paginate the small bundled list in
  memory;
- workspaces use `created_at DESC, id DESC`;
- runtime operations use `created_at DESC, id DESC` and optionally filter by
  `workspace_id`.

Workspace and operation pages use a count query plus a paginated data query.
The secondary ID ordering makes pages deterministic when timestamps match.

## UI-Safe DTOs

Facade DTOs are projections, not aliases for application models.

`WorkspaceDto` exposes:

- workspace ID;
- workflow reference;
- creation time;
- optional tagged `RuntimeDto`.

`RuntimeDto` exposes the shared state and a tagged provider projection. The
RunPod projection exposes placement configuration. Network volume, provisioner
pod, template, and endpoint IDs remain native-only because the renderer neither
needs nor owns provider resources.

`RuntimeOperationDto` exposes operation identity, workspace identity, runtime
kind, operation kind, state, trace ID, resolved provider progress, and
timestamps. Its tagged progress shape keeps invalid combinations such as a
provision operation with a cleanup step unrepresentable.

`IdentityDto` exposes only the safe optional key name, username, and email.
Raw credentials never appear in output DTOs, events, errors, logs, generated
types, fixtures, or persisted workspace data.

Application models do not derive Specta or transport serialization solely for
the facade. Conversion is explicit at the inbound boundary.

## Events

The application event surface is reduced to the complete snapshots needed by
the facade:

```rust
pub enum ApplicationEvent {
    WorkspaceChanged(Workspace),
    WorkspaceDeleted { workspace_id: String },
    RuntimeOperationChanged(RuntimeOperation),
}
```

Tauri Specta exports three events:

| Event | Payload |
| --- | --- |
| `workspace_changed` | `{ workspace: WorkspaceDto }` |
| `workspace_deleted` | `{ workspace_id }` |
| `runtime_operation` | `{ operation: RuntimeOperationDto }` |

After every durable provision or cleanup transition, event order is:

```text
commit workspace runtime + operation
-> workspace_changed
-> runtime_operation
-> continue detached work or return
```

Workspace create emits `workspace_changed` after commit. Workspace delete emits
`workspace_deleted` after commit. Failed or rejected writes emit nothing.

The Tauri event sink maps and emits synchronously. Delivery remains best-effort:
an emit failure is logged safely and never rolls back persistence, changes an
operation state, or starts a retry. There is no event relay, outbox, sequence
number, or persisted workspace revision.

A detached provider step may publish a newer event before JavaScript processes
the initial mutation response. The frontend workspace store therefore treats
events as authoritative snapshots; a mutation response must not overwrite a
newer event already applied by the store.

## Errors And Diagnostics

Every command uses a shared envelope with a command-specific code enum:

```rust
pub type CommandResult<T, Code> = Result<T, CommandError<Code>>;

pub struct CommandError<Code> {
    pub code: Code,
    pub trace_id: String,
}
```

Each error-code enum contains only errors possible for that command plus a
`command_error` fallback for an unexpected boundary-mapping failure. Error
families include:

- catalog availability;
- workspace, workflow, and runtime state conflicts;
- missing credentials and invalid credentials;
- provider, persistence, identity, and storage unavailability;
- invalid pagination;
- invalid runtime transitions.

The runtime persistence boundary uses only shared persistence errors:

```rust
pub enum RuntimePersistenceError {
    AlreadyExists,
    OperationAlreadyRunning,
    NotFound,
    Unavailable,
    CorruptData,
}
```

An unknown stored `runtime_kind`, a missing provider extension, or a mismatch
between `Runtime.provider.kind()` and `RuntimeOperation.runtime_kind` is
`CorruptData`. Provider persistence maps SeaORM errors into this shared
boundary and never exposes raw database or provider messages. There are no
cross-provider fallback reads.

Provider, SQLite, bundled-catalog, and keyring messages are never returned to
the renderer. The client receives a stable code and the root command trace ID.
Application UUIDs are converted to strings in facade DTOs.

Every command is annotated as an explicit diagnostics root. Safe identifiers,
pagination fields, and selected output DTOs may be shown through
`DiagnosticValue`; API-key inputs are always redacted. No command body creates
or propagates spans manually.

Provision and cleanup return after their initial transition commits. A later
provider failure is not a second command error: the detached runner persists a
failed workspace runtime and operation, publishes both events, and leaves
technical details in `diagnostics.log` under the operation trace ID.

## Tauri Builder And Code Generation

The facade owns a Tauri Specta builder that collects every command and the three
events. The same builder supplies the invoke handler, mounts events, and exports
`src/generated/commands.ts` in the existing `export_bindings` test.

Generated TypeScript is never edited manually. Command, event, or DTO contract
changes run `bun run codegen:commands`.

## `lib.rs` Wiring

`lib.rs` is the single composition root. Startup order is:

1. construct the Tauri Specta builder with commands and events;
2. construct the Tauri builder and existing plugins;
3. mount Specta events before any recovery can emit them;
4. resolve and create `app_data_dir`;
5. initialize diagnostics at `app_data_dir/diagnostics.log`;
6. connect and schema-sync SQLite at `app_data_dir/db.sqlite`;
7. resolve the packaged bundled catalog at `resource_dir/bundled`;
8. construct bundled, SQLite, keyring, RunPod, and Hugging Face adapters,
   including provider persistence implementations and the closed SQLite
   persistence dispatcher;
9. construct application services, the concrete facade runtime dispatcher, the
   Tauri event sink, and `FacadeState`;
10. fail interrupted runtime operations while events are already mounted;
11. manage the fully initialized facade state;
12. finish Tauri startup.

The bundled directory is added to Tauri bundle resources. The opener plugin
remains enabled, and the MCP bridge remains debug-only.

Path, diagnostics, schema, adapter construction, or interrupted-recovery
failure aborts startup. No command is exposed against partially initialized
state, and no startup-status command is added.

## Testing And Verification

Focused tests cover the new behavior at its owning boundary:

- SQLite integration tests cover shared state in the generic anchor, provider
  dispatch, batch-hydrated workspace pages, totals and stable ordering, atomic
  rollback on provider failure, and operation progress hydration after cleanup;
- application tests cover generated workspace UUIDs, full workspace events,
  pagination behavior, and updated provision/cleanup transitions;
- facade tests cover tagged DTO conversion, secret omission, command-specific
  error mapping, pagination validation, and provider-service dispatch without
  provider workflow logic in the dispatcher;
- a focused architecture audit confirms that generic runtime repositories
  contain no RunPod SQL, field mapping, lifecycle branches, or resource fields
  outside exhaustive dispatch arms;
- the binding export test covers command/event collection and generated
  TypeScript output;
- bootstrap tests cover support filenames and ensure events are mounted before
  interrupted-operation recovery.

Required verification from the repository root:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
bun run codegen:commands
bun run build
bun run lint
```

## Explicit Deferrals

- No multiple runtime relation or `Workspace::runtimes` collection.
- No dispatcher trait, dynamic provider registry, or plugin loading.
- No provider SQL, mapping, lifecycle behavior, or resource fields in generic
  runtime repositories; provider names may appear there only in exhaustive
  dispatch arms.
- No workspace revision solely to order command responses against events.
- No pagination cursor; the approved contract uses offset, limit, and total.
- No secret-status command.
- No compatibility layer for the pre-v1 runtime schema change.
