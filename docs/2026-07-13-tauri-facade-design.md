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
- concrete runtime dispatch, initially with one RunPod arm;
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
- `Runtime` is a tagged application enum with an initial `Runpod` variant.
- The `workspace_runtimes` anchor table is removed.
- `runpod_workspace_runtimes.workspace_id` is a direct primary-key/foreign-key
  relation to `workspaces.id`.
- Cross-provider exclusivity remains an application invariant until a second
  provider creates a concrete need for a generic anchor.
- The facade uses a concrete runtime dispatcher with an ordinary enum match.
  There is no dispatcher trait, factory, registry, or plugin system.
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
```

The provider runtime does not repeat `workspace_id`; its identity comes from
the parent workspace. The RunPod variant continues to own its provider state,
placement configuration, and native-only resource IDs.

`WorkspaceService::create` generates the workspace UUID inside the application
boundary. `CreateWorkspaceRequest` therefore contains only the workflow
reference.

### Direct SQLite Relation

The persisted shape is:

```text
workspaces
  id PK
  workflow_id
  workflow_revision
  created_at

runpod_workspace_runtimes
  workspace_id PK/FK -> workspaces.id ON DELETE CASCADE
  state
  datacenter_id
  gpu_id
  volume_size_gb
  provider resource IDs
```

`workspace_runtimes` and its SeaORM entity are deleted directly. This is a
pre-v1 schema change; no migration, compatibility read, or fallback is added.

`WorkspaceRepository::get` and the paginated workspace read load the base row
and its optional provider extension as one hydrated application `Workspace`.
With RunPod as the only provider this is one joined read, not an N+1 facade
resolver.

A provider extension without its workspace is prevented by the foreign key. A
workspace with more than one provider extension is prevented by application
transition logic until another provider requires a database-level solution.

### Runtime Transitions

Runtime transition persistence continues to atomically write the provider
runtime and `RuntimeOperation`. Provider services mutate the runtime nested in
the workspace snapshot and publish that complete workspace only after the
transition commits.

Provision inserts the RunPod extension. Successful cleanup deletes it and
produces `Workspace { runtime: None }`. Workspace deletion remains rejected
while a runtime is attached or an operation is running. Runtime-operation
history continues to survive workspace deletion.

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

The dispatcher has one concrete RunPod service field.

- `provision_workspace` matches the tagged runtime request and calls the
  corresponding provider service.
- `cleanup_workspace` loads the workspace once, matches its hydrated runtime,
  and calls the corresponding provider service.
- interrupted-operation recovery calls the RunPod recovery entry point.

Adding another provider adds one enum variant, one service field, and match
arms. No abstraction is introduced before that provider exists.

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

The RunPod runtime DTO exposes state and placement configuration. Network
volume, provisioner pod, template, and endpoint IDs remain native-only because
the renderer neither needs nor owns provider resources.

`RuntimeOperationDto` exposes operation identity, workspace identity, kind,
state, trace ID, resolved provider progress, and timestamps. Its tagged progress
shape keeps invalid combinations such as a provision operation with a cleanup
step unrepresentable.

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
8. construct bundled, SQLite, keyring, RunPod, and Hugging Face adapters;
9. construct application services, the concrete runtime dispatcher, the Tauri
   event sink, and `FacadeState`;
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

- SQLite integration tests cover the direct workspace-to-RunPod foreign key,
  hydrated workspace pages, totals and stable ordering, and atomic
  runtime-operation transitions;
- application tests cover generated workspace UUIDs, full workspace events,
  pagination behavior, and updated provision/cleanup transitions;
- facade tests cover tagged DTO conversion, secret omission, command-specific
  error mapping, pagination validation, and the concrete RunPod dispatcher
  arms;
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
- No generic runtime anchor until a second provider makes cross-provider
  exclusivity concrete.
- No dispatcher trait, provider registry, or dynamic provider loading.
- No workspace revision solely to order command responses against events.
- No pagination cursor; the approved contract uses offset, limit, and total.
- No secret-status command.
- No compatibility layer for the removed `workspace_runtimes` schema.
