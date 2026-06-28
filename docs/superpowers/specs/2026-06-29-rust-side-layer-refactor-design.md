# Rust-Side Layer Refactor Design

## Context

LumaForge's current native backend is organized around `tauri_api`, `app`,
`workspace`, `domain`, `provider`, catalog modules, SQLite repositories, and
secret storage. The target architecture is the Rust-side brief in
`.prompts/rust-side-architecture-brief.md`.

The current SQLite implementation stores provider-specific runtime and
operation state as `runtime_json` and `payload_json`. The target contract
normalizes that state into runtime-specific tables and keeps SeaORM inside the
SQLite infrastructure layer.

## Approved Direction

Use a persistence-first, iterative refactor.

The final architecture follows the full brief. This spec is an umbrella design:
it preserves shared context, target boundaries, and iteration order. It is not
detailed enough to implement any iteration directly.

Each broad iteration requires its own focused design spec before implementation
planning. That iteration spec must define the exact layer contract, module/file
targets, repository or port API, test scope, and verification commands for that
iteration. Only after that focused spec is approved should an implementation
plan be written for that iteration.

Intermediate iterations do not need to keep the complete backend runnable. That
is intentional: preserving full app behavior at every step would push the
refactor toward legacy bridges and compatibility shims. During an iteration,
neighboring layers may be temporarily broken if the layer being changed remains
testable through its own focused tests. Full backend integration is restored in
the final iteration.

No migration or compatibility path is added for the old pre-v1 JSON persistence
schema. A new database uses the target schema. An incompatible existing dev
database should fail clearly instead of falling back to old columns.

## Iteration Sequence

### Iteration 1: Persistence

Add SeaORM and introduce `infra/sqlite/entities/*` plus
`infra/sqlite/repositories/*`. Replace the JSON-backed workspace and lifecycle
repositories with normalized relational repositories.

Targets:

- `workspaces`
- `workspace_runtimes`
- `runpod_workspace_runtimes`
- `lifecycle_operations`
- `runpod_operation_payloads`

Remove `runtime_json` and `payload_json` from the target persistence contract.

### Iteration 2: Bundled Catalogs

Move bundled workflow and runtime catalog loading into `infra/catalogs`.

Targets:

- `infra/catalogs/workflows.rs`
- `infra/catalogs/runtime_contracts.rs`
- `infra/catalogs/execution_schemas.rs`
- `infra/catalogs/errors.rs`
- `infra/catalogs/mod.rs`

`infra/catalogs` owns reading bundled JSON, validating catalog shape, and
returning catalog data through a persistence-free API. Bundled JSON remains
static app data under `bundled/**`.

This iteration gets its own focused design spec before implementation planning.
That spec must define the catalog data API, validation boundary, error shape,
and how later application ports will consume catalog data.

### Iteration 3: Infra Keyring And Providers

Move secure storage and raw provider HTTP clients into infrastructure modules.

Targets:

- `infra/keyring/*`
- `infra/providers/runpod/*`
- `infra/providers/hugging_face/*`

`infra/keyring` owns technical secure storage through the platform keyring.
`infra/providers/runpod` and `infra/providers/hugging_face` own raw HTTP
clients, provider request/response mapping, and provider identity calls.

This iteration does not implement application ports. It prepares concrete
storage and provider primitives that later adapter layers can use.

### Iteration 4: Application

Move provider-neutral models and ports into `application`.

Targets:

- `application/model.rs`
- `application/ports.rs`
- `application/errors.rs`
- `application/workspace_service.rs`
- `application/lifecycle_runner.rs`

`application` owns workspace use cases, lifecycle operation creation, state
transitions, in-flight operation tracking, and provider-neutral ports. It does
not import Tauri, SeaORM, SQLx, reqwest, keyring, or concrete provider clients.
Credential-facing capabilities are declared as application ports here, but
their implementations live outside `application`.

### Iteration 5: Secrets

Create a top-level `secrets` adapter layer.

Targets:

- `secrets/model.rs`
- `secrets/errors.rs`
- `secrets/runpod.rs`
- `secrets/hugging_face.rs`
- `secrets/mod.rs`

`secrets` implements application credential ports by composing
`infra/keyring` and `infra/providers/*`. It owns credential workflows such as
setup, delete, identity lookup, trusted secret retrieval, and RunPod workspace
bearer token issuing. RunPod workspace bearer token issuing lives in
`secrets/runpod.rs`, not a separate module. It does not expose raw secrets to
`facade` or React.

### Iteration 6: RunPod Runtime

Move RunPod-specific lifecycle orchestration into `runtime/runpod`.

Targets:

- `runtime/runpod/model.rs`
- `runtime/runpod/ports.rs`
- `runtime/runpod/provision.rs`
- `runtime/runpod/cleanup.rs`
- `runtime/runpod/delete.rs`

RunPod lifecycle code owns step order, cleanup order, polling behavior, and
RunPod payload updates. It reports progress through ports and does not import
SQLite or Tauri.

### Iteration 7: Facade And Composition

Move Tauri/Specta API boundaries into `facade` and concrete dependency wiring
into `composition`.

Targets:

- `facade/commands/*`
- `facade/events.rs`
- `facade/types/*`
- `facade/errors.rs`
- `facade/tracing.rs`
- `composition/bootstrap.rs`
- `composition/state.rs`

Tauri command DTOs and generated frontend bindings may change. The generated
`src/generated/commands.ts` file is updated through codegen, not manually.

## Target Dependency Rule

The final dependency direction is:

```text
facade -> application
application -> models + ports
runtime/runpod -> models + ports
secrets -> application ports + infra/keyring + infra/providers
infra -> models + ports
composition -> wires all
```

Layer ownership:

- `facade`: Tauri commands/events, Specta DTOs, UI-safe errors, `traceId`.
- `application`: provider-neutral workspace use cases and lifecycle state.
- `runtime/runpod`: RunPod lifecycle sequence and RunPod-specific runtime data.
- `infra`: SeaORM SQLite repositories, bundled catalog readers, keyring, HTTP
  clients, and Tauri event sink implementation.
- `secrets`: credential workflows that implement application ports using
  keyring and provider infrastructure.
- `composition`: database open, diagnostics init, concrete dependency wiring,
  and `NativeAppState`.

There is no separate `domain` layer in the final target. Provider-neutral models
live in `application/model.rs`. RunPod-specific models live in
`runtime/runpod/model.rs`.

## Persistence Contract

Target SQLite schema:

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
- SeaORM entities stay under `infra/sqlite/entities`.
- SeaORM repositories stay under `infra/sqlite/repositories`.
- `application`, `runtime/runpod`, and `facade` never import SeaORM `Entity`,
  `Model`, or `ActiveModel`.
- Transactions enforce parent and child writes for workspace/runtime rows and
  operation/payload rows.

## Application And Runtime Flow

Provider-neutral application models:

```text
Workspace { id, workflow, state }
LifecycleOperation { id, workspace_id, kind, state, timestamps }
```

RunPod runtime models:

```text
RunpodWorkspaceRuntime { workspace_id, placement, resources }
RunpodOperationPayload { operation_id, step }
```

Application ports:

```text
WorkspaceRepository
LifecycleOperationRepository
WorkflowCatalogRepository
RuntimeCatalogRepository
WorkspaceEventSink
WorkspaceRuntimeLifecycle
```

`WorkspaceRuntimeLifecycle` is the application-owned trait used by
`WorkspaceService` to run provision, cleanup, and delete operations without
knowing the concrete runtime implementation.

RunPod runtime ports:

```text
RunpodRuntimeClient
```

Secret-related application ports are declared in `application/ports.rs` and
implemented in the top-level `secrets` layer. Low-level keyring and provider
clients remain in `infra`.

Provision flow:

```text
facade command
  -> application::WorkspaceService::provision_workspace
  -> create lifecycle operation and set workspace Provisioning
  -> spawn lifecycle runner
  -> runtime::runpod::provision
  -> report step/resource updates through application-owned progress port
  -> application repositories persist normalized rows
  -> event sink emits UI-safe events
```

Application owns operation terminal state and workspace state transitions.
Runtime reports RunPod facts and requested resource updates. Infrastructure
persists those updates through application-owned repository ports.

## Facade, Errors, And Secrets

The facade may change command DTOs and events. React-facing data remains
UI-safe:

- no raw provider API keys
- no bearer tokens
- no worker tokens
- no credential-bearing URLs
- no raw provider payloads
- no SQLite internals

Errors are mapped at boundaries:

```text
infra/provider errors -> application/runtime errors
application/runtime errors -> facade CommandError { code, message, traceId }
```

`CommandError` remains UI-safe and traceable. `traceId` is created at the
command boundary or startup context. Logs may include structured diagnostics,
but must not include secrets.

Secrets remain write-only from React. Facade commands may set, delete, and
validate credential identity, but cannot return raw secret values. Keyring
access lives in `infra/keyring`. Credential workflows live in top-level
`secrets`. Trusted runtime/provider paths receive secret material only through
application ports implemented by `secrets`.

## Testing And Verification

Each broad iteration gets its own focused design spec, implementation plan, and
verification commands.

Iteration 1 verifies the persistence layer only:

- SeaORM schema/entities bootstrap.
- Workspace plus runtime child mapping.
- Lifecycle operation plus RunPod payload child mapping.
- Repository transactions.
- No `runtime_json` or `payload_json` in the target schema.

Iteration 2 verifies bundled catalog loading and validation boundaries only.

Iteration 3 verifies keyring/provider infrastructure boundaries only.

Iteration 4 verifies application models, ports, and use cases with fake ports.

Iteration 5 verifies secrets adapters against fake keyring/provider
infrastructure.

Iteration 6 verifies `runtime/runpod` step sequencing and provider failure
handling with fake provider/progress ports.

Iteration 7 verifies facade, composition, codegen, and full backend integration.

Full final verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
bun run codegen:commands
bun run build
bun run lint
```

Do not add tests for removed JSON persistence, deprecated module names,
compatibility paths, old command DTOs, or absence of removed behavior.
