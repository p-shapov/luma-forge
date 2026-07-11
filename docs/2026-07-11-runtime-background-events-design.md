# Runtime Background Execution and Application Events Design

**Date:** 2026-07-11

## Goal

Restore the earlier lifecycle execution behavior on the current application
models and ports:

- persist an initial lifecycle transition before returning from a start call;
- run provider work in a detached Tokio task;
- publish application events after every durable runtime/lifecycle transition;
- publish workspace projection events when a runtime is attached or detached;
- keep background execution and event publication reusable when another runtime
  provider is added.

This iteration ends at the application boundary. Tauri, inbound DTOs, Specta,
generated frontend contracts, frontend state, and a concrete event sink are not
part of the scope.

## Principles

- Application code uses the models under `application/**/model.rs` directly.
- Application models are not Tauri or UI transport contracts.
- A future inbound adapter decides which application fields are exposed.
- SQLite is authoritative. Events are best-effort notifications after commit.
- Provider-specific code owns provider steps, not background or event mechanics.
- The initial `LifecycleOperation` UUID is the background operation handle; no
  separate job ID exists.
- No cancellation, retry framework, resume, reconciliation, outbox, polling
  event pump, or in-memory task registry is added.

## Application Runtime Model

Add a provider-neutral application runtime enum:

```rust
pub enum Runtime {
    Runpod(RunpodRuntime),
}
```

Provider runtime models implement a small common contract:

```rust
pub trait RuntimeModel: Clone + Send + Sync + 'static {
    fn workspace_id(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn into_runtime(self) -> Runtime;
}
```

Adding another provider adds its application model, a `Runtime` variant, and a
`RuntimeModel` implementation. Existing background and event code does not gain
provider branches.

Provider-owned resource IDs remain in the provider runtime model because native
cleanup and recovery require them. A future inbound adapter may omit those IDs
from its public runtime DTO.

## Application Events

Application events carry application models, not facade DTOs:

```rust
pub enum ApplicationEvent {
    WorkspaceChanged(Workspace),
    WorkspaceDeleted {
        workspace_id: String,
    },
    RuntimeChanged(Runtime),
    RuntimeDeleted {
        workspace_id: String,
        kind: RuntimeKind,
    },
    LifecycleOperationChanged(LifecycleOperation),
}

pub trait ApplicationEventSink: Send + Sync {
    fn emit(&self, event: ApplicationEvent);
}
```

`ApplicationEventSink::emit` returns no result. A future concrete adapter owns
delivery-specific error handling. Delivery failure never rolls back persistence,
changes lifecycle state, or starts a retry.

### Event Ordering

Events are emitted synchronously in a deterministic order after a successful
atomic transition commit.

Ordinary runtime progress, terminal provision success, and failed transitions:

```text
RuntimeChanged
LifecycleOperationChanged
```

Initial runtime attachment:

```text
WorkspaceChanged
RuntimeChanged
LifecycleOperationChanged
```

Successful runtime cleanup and detachment:

```text
WorkspaceChanged
RuntimeDeleted
LifecycleOperationChanged
```

Every durable provider step publishes runtime and lifecycle snapshots. Workspace
events are not repeated for ordinary progress because the workspace projection
does not change.

`WorkspaceService` uses the same sink:

- successful create emits `WorkspaceChanged`;
- successful delete emits `WorkspaceDeleted`;
- rejected or failed writes emit nothing.

## Generic Runtime Transition Context

The shared transition context owns persistence-to-event ordering. It is generic
over a provider runtime model and its transition repository.

```rust
#[async_trait]
pub trait RuntimeTransitionRepository<R: RuntimeModel>: Send + Sync {
    async fn save_transition(
        &self,
        runtime: &R,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError>;
}

pub struct RuntimeTransitionContext<R: RuntimeModel> {
    transitions: Arc<dyn RuntimeTransitionRepository<R>>,
    workspaces: Arc<dyn WorkspaceRepository>,
    events: Arc<dyn ApplicationEventSink>,
}
```

The context exposes three semantic operations:

- `save_changed`: persist, then emit runtime and lifecycle changes;
- `save_attached`: persist, reload the workspace projection, then emit the
  workspace, runtime, and lifecycle changes;
- `save_deleted`: persist the cleanup terminal transition, reload the detached
  workspace projection, then emit the workspace, runtime deletion, and lifecycle
  changes.

The current RunPod transition repository implements the generic write port.
Provider-specific reads remain on `RunpodRuntimeRepository`. A future provider
implements the same generic write port for its own runtime model; it does not
reimplement the transition context.

Workspace reloads happen only after persistence succeeds, so emitted workspace
models reflect the committed runtime anchor relation.

## Background Runner

The background runner is provider-neutral and contains no Tauri, repository,
event, runtime-model, or provider API knowledge:

```rust
#[derive(Clone)]
pub struct LifecycleBackgroundRunner;

impl LifecycleBackgroundRunner {
    pub fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(task);
    }
}
```

The initial operation and runtime transition must commit before `spawn` is
called. Only the caller that successfully commits the initial transition starts
the task. The SQLite one-running-operation invariant rejects concurrent starts,
so no separate in-memory registry is required.

If the process stops or the task panics after the initial commit, the existing
startup recovery finds the remaining `Running` operation and atomically marks it
and its runtime `Failed`. Provider reconciliation or execution resume is not
attempted.

## RunPod Start and Execution Flow

`RunpodRuntimeService` becomes cloneable and owns its dependencies through
`Arc`, allowing a clone and prepared provider inputs to move into a detached
future.

Its public lifecycle entry points become:

```rust
async fn start_provision(
    &self,
    command: ProvisionRunpodRuntime,
) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>;

async fn start_cleanup(
    &self,
    workspace_id: &str,
) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>;
```

### Provision Start

`start_provision` performs all preflight work before durable mutation:

1. load and validate the workspace;
2. reject an attached or failed/in-progress runtime as required by the current
   state model;
3. reject an existing running lifecycle operation;
4. resolve workflow and RunPod runtime contracts;
5. load every required credential;
6. build the initial `Provisioning` runtime and `Running` operation;
7. commit through `save_attached` and publish initial events;
8. spawn the prepared provider future;
9. return the initial runtime and operation snapshots immediately.

No provider API call occurs before the detached task begins.

The detached RunPod provision flow retains its current six provider steps. It
sets and persists each step before the corresponding provider call. Each
successful transition uses `save_changed`. Provider failure marks the runtime
and operation `Failed` through the same context. Terminal success marks the
runtime `Ready` and operation `Succeeded` through `save_changed`.

### Cleanup Start

`start_cleanup` performs all preflight work, including loading the RunPod
credential, before durable mutation. It then:

1. creates the initial `CleaningUp` runtime and `Running` cleanup operation;
2. commits through `save_changed` and publishes initial events;
3. spawns the prepared cleanup future;
4. returns the initial runtime and operation snapshots immediately.

Cleanup persists every cleanup step before its optional provider call. Provider
failure persists `Failed` runtime/operation snapshots with `save_changed`.
Successful cleanup uses `save_deleted`, retaining lifecycle history while
deleting the runtime anchor and provider extension.

### Persistence Failure

If provider work fails and the failed transition commits, the journal is the
complete terminal result and the detached task ends normally.

If transition persistence itself fails, the task stops and performs no further
provider calls. The last durable operation may remain `Running`; startup recovery
handles it on the next process start. There is no retry or compensating write.

## Startup Recovery

`fail_interrupted` keeps its existing behavior but saves recovered transitions
through `RuntimeTransitionContext`. Each recovered operation therefore emits:

```text
RuntimeChanged(Failed)
LifecycleOperationChanged(Failed)
```

Progress and trace ID remain unchanged. Recovery makes no provider calls and
does not spawn background work.

## Testing

Behavioral tests remain under `application`. No adapter, SQLite integration,
Tauri, Specta, generated-contract, or frontend tests are added in this scope.

### Detached Execution

A fake provider blocks its first call on a Tokio synchronization primitive. The
test verifies that:

- `start_provision` returns while the provider remains blocked;
- the initial runtime and operation are already durable;
- initial events were already emitted;
- releasing the provider allows the detached flow to finish.

Tests use `Notify` or channels, not timing sleeps.

### Event Sequences

A recording `ApplicationEventSink` verifies exact event contents and ordering
for:

- initial provision attachment;
- every provision progress step;
- provision success and provider failure;
- cleanup start and every cleanup progress step;
- cleanup success/deletion and cleanup failure;
- workspace create and delete;
- interrupted startup recovery.

Repository fakes record committed snapshots before the sink records events,
allowing tests to prove that no event precedes its durable transition.

### Rejected Operations

Preflight errors, duplicate starts, missing credentials, and persistence errors
before a successful commit do not spawn tasks or emit events.

## Extensibility

Adding a runtime provider requires:

1. its provider-specific application model and progress steps;
2. a `Runtime` enum variant and `RuntimeModel` implementation;
3. its provider-specific repository and provider ports/adapters;
4. its provider workflow implementation using `RuntimeTransitionContext`;
5. provider-specific behavioral tests.

It does not require changes to:

- `LifecycleBackgroundRunner`;
- `RuntimeTransitionContext` persistence/event ordering;
- `ApplicationEventSink`;
- workspace attach/detach event semantics;
- generic background error behavior.

## Explicitly Out of Scope

- Tauri setup, commands, managed state, or event mounting;
- facade/inbound DTOs;
- Specta derives and command/event code generation;
- generated frontend contracts or frontend state integration;
- a concrete application event sink;
- tracing and diagnostics wiring;
- cancellation and task management APIs;
- retry, resume, reconciliation, or provider compensation;
- an event outbox or database polling event pump;
- multiple simultaneous runtimes per workspace;
- a second provider implementation solely to demonstrate extensibility.
