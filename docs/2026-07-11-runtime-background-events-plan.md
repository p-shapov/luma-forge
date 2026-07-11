# Runtime Background Execution and Application Events Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Start RunPod provision and cleanup as detached application tasks after a durable initial transition, and publish provider-neutral application events after every committed workspace/runtime/lifecycle change.

**Architecture:** Application-owned `Runtime`, `ApplicationEvent`, and `RuntimeTransitionContext` provide reusable runtime transition and event mechanics. RunPod retains provider-specific sequencing, starts detached work directly with `tokio::spawn`, and calls the generic transition context before every provider operation; SQLite remains authoritative and event delivery is best-effort after commit.

**Tech Stack:** Rust 2021, Tokio multi-thread runtime and synchronization primitives, `async-trait`, SeaORM SQLite, existing application models/ports/adapters.

**Design:** `docs/2026-07-11-runtime-background-events-design.md`

## Global Constraints

- Work only in the active `src-tauri/src` tree plus `src-tauri/Cargo.toml`; never read from, import, compile, copy, or commit `src-tauri/old_src`.
- Application code uses the models under `application/**/model.rs`; application models do not gain Tauri, Specta, serde facade, or generated-contract dependencies.
- Do not add Tauri setup, commands, managed state, event mounting, inbound DTOs, Specta derives, codegen, frontend changes, or a concrete event sink.
- SQLite is authoritative. Emit application events synchronously and best-effort only after a successful transition commit.
- Event delivery failure never rolls back persistence, changes lifecycle state, or starts a retry.
- The operation UUID is the background handle; do not add a job ID, task registry, cancellation API, retry, resume, reconciliation, outbox, or polling event pump.
- Initial provision/cleanup transitions commit before `tokio::spawn`; no provider API call occurs before the detached task begins.
- Every durable runtime transition emits a runtime event followed by a lifecycle event. Runtime attach/detach additionally emits the workspace projection first.
- Keep transition and event mechanics provider-neutral. Provider workflows start their detached Tokio tasks directly; a future provider adds a runtime model/enum variant/repository/workflow, not a new context or sink.
- Keep provider resource IDs inside provider-specific application models; this scope adds no public DTO.
- Add behavioral tests only under `application`; do not add adapter, SQLite integration, Tauri, Specta, generated-contract, or frontend tests.
- Use Tokio synchronization primitives in detached-task tests; do not use timing sleeps.
- Run focused RED/GREEN tests for each task. Every task ends with `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and an appropriate native test/check gate.
- Commit only the files listed in each task. Use Conventional Commits.

---

## File Structure

### New application files

- `src-tauri/src/application/events.rs`: application event enum and sink port.
- `src-tauri/src/application/runtimes/model.rs`: provider-neutral `Runtime` and `RuntimeModel`.
- `src-tauri/src/application/runtimes/ports/mod.rs`: generic runtime port exports.
- `src-tauri/src/application/runtimes/ports/runtime_transition_repository.rs`: generic transition write port and typed errors.
- `src-tauri/src/application/runtimes/transition.rs`: commit-then-event transition context.

### Existing application files

- `src-tauri/src/application/runtimes/runpod/model.rs`: implement `RuntimeModel` for RunPod.
- `src-tauri/src/application/runtimes/runpod/ports/runtime_repository.rs`: keep RunPod reads and inherit the generic transition write port.
- `src-tauri/src/application/workspace/service.rs`: emit workspace create/delete events.
- `src-tauri/src/application/runtimes/runpod/service.rs`: preflight, durable start, detached RunPod execution, recovery.
- `src-tauri/src/application/runtimes/runpod/test_support.rs`: Arc-owned fakes, recording events, deterministic provider gate.

### Existing adapter/config files

- `src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs`: implement generic transition writes plus RunPod reads.
- `src-tauri/Cargo.toml`: explicitly enable Tokio `sync` for deterministic tests.

---

### Task 1: Provider-Neutral Runtime and Application Event Contracts

**Files:**
- Create: `src-tauri/src/application/events.rs`
- Create: `src-tauri/src/application/runtimes/model.rs`
- Modify: `src-tauri/src/application/mod.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/model.rs`

**Interfaces:**
- Consumes: current `Workspace`, `RuntimeKind`, `RunpodRuntime`, and `LifecycleOperation` models.
- Produces: `Runtime`, `RuntimeModel`, `ApplicationEvent`, and `ApplicationEventSink`.

- [ ] **Step 1: Add the failing provider-neutral runtime test**

Create `application/runtimes/model.rs` with the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::runpod::{
        RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeResources, RunpodRuntimeState,
    };

    #[test]
    fn runpod_model_converts_without_erasing_its_provider_type() {
        let runtime = RunpodRuntime {
            workspace_id: "workspace-1".into(),
            state: RunpodRuntimeState::Ready,
            config: RunpodRuntimeConfig {
                datacenter_id: "dc-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 19,
            },
            resources: RunpodRuntimeResources::default(),
        };

        assert_eq!(runtime.workspace_id(), "workspace-1");
        assert_eq!(runtime.kind(), RuntimeKind::Runpod);
        assert_eq!(runtime.clone().into_runtime(), Runtime::Runpod(runtime));
    }
}
```

Add the module shells:

```rust
// application/runtimes/mod.rs
mod model;
pub mod runpod;

pub use model::{Runtime, RuntimeModel};

// application/mod.rs
pub mod events;
```

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::model::tests::runpod_model_converts_without_erasing_its_provider_type -- --exact
```

Expected: compilation fails because `Runtime`, `RuntimeModel`, and the RunPod implementation do not exist.

- [ ] **Step 3: Implement `Runtime` and `RuntimeModel`**

Add to `application/runtimes/model.rs`:

```rust
use crate::application::workspace::RuntimeKind;

use super::runpod::RunpodRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    Runpod(RunpodRuntime),
}

pub trait RuntimeModel: Clone + Send + Sync + 'static {
    fn workspace_id(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn into_runtime(self) -> Runtime;
}
```

Implement it in `application/runtimes/runpod/model.rs`:

```rust
use crate::application::{
    runtimes::{Runtime, RuntimeModel},
    workspace::{RuntimeKind, WorkspaceStatus},
};

impl RuntimeModel for RunpodRuntime {
    fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Runpod
    }

    fn into_runtime(self) -> Runtime {
        Runtime::Runpod(self)
    }
}
```

Keep the existing `From<RunpodRuntimeState> for WorkspaceStatus` implementation unchanged.

- [ ] **Step 4: Add application event contracts**

Create `application/events.rs`:

```rust
use crate::application::{
    lifecycle::LifecycleOperation,
    runtimes::Runtime,
    workspace::{RuntimeKind, Workspace},
};

#[derive(Debug, Clone, PartialEq, Eq)]
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

Do not add a no-op or concrete sink.

- [ ] **Step 5: Run GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::model::tests::runpod_model_converts_without_erasing_its_provider_type -- --exact
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: the focused test passes and formatting exits 0.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/mod.rs src-tauri/src/application/events.rs src-tauri/src/application/runtimes/mod.rs src-tauri/src/application/runtimes/model.rs src-tauri/src/application/runtimes/runpod/model.rs
git commit -m "feat(application): add runtime event contracts"
```

---

### Task 2: Generic Commit-Then-Event Transition Context

**Files:**
- Create: `src-tauri/src/application/runtimes/ports/mod.rs`
- Create: `src-tauri/src/application/runtimes/ports/runtime_transition_repository.rs`
- Create: `src-tauri/src/application/runtimes/transition.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/ports/runtime_repository.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/ports/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/errors.rs`
- Modify: `src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs`

**Interfaces:**
- Consumes: Task 1 `RuntimeModel`/events, `WorkspaceRepository`, and current atomic RunPod persistence.
- Produces: `RuntimeTransitionRepository<R>`, `RuntimeTransitionRepositoryError`, and `RuntimeTransitionContext<R, P>::{save_changed,save_attached,save_deleted}`.

- [ ] **Step 1: Write failing transition ordering tests**

Create `application/runtimes/transition.rs` with tests using local fakes. The core assertions must be:

```rust
#[tokio::test]
async fn attached_transition_commits_before_ordered_events() {
    let fakes = Fakes::attached();

    fakes.context().save_attached(&fakes.runtime, &fakes.operation).await.unwrap();

    assert_eq!(fakes.events.events(), vec![
        ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
        ApplicationEvent::RuntimeChanged(Runtime::Runpod(fakes.runtime.clone())),
        ApplicationEvent::LifecycleOperationChanged(fakes.operation.clone()),
    ]);
    assert!(fakes.events.all_emitted_after_commit());
}

#[tokio::test]
async fn changed_transition_emits_only_runtime_then_lifecycle() {
    let fakes = Fakes::attached();

    fakes.context().save_changed(&fakes.runtime, &fakes.operation).await.unwrap();

    assert_eq!(fakes.events.events(), vec![
        ApplicationEvent::RuntimeChanged(Runtime::Runpod(fakes.runtime.clone())),
        ApplicationEvent::LifecycleOperationChanged(fakes.operation.clone()),
    ]);
}

#[tokio::test]
async fn deleted_transition_emits_detached_workspace_before_deletion_and_lifecycle() {
    let fakes = Fakes::detached();

    fakes.context().save_deleted(&fakes.runtime, &fakes.operation).await.unwrap();

    assert_eq!(fakes.events.events(), vec![
        ApplicationEvent::WorkspaceChanged(fakes.workspace.clone()),
        ApplicationEvent::RuntimeDeleted {
            workspace_id: "workspace-1".into(),
            kind: RuntimeKind::Runpod,
        },
        ApplicationEvent::LifecycleOperationChanged(fakes.operation.clone()),
    ]);
}

#[tokio::test]
async fn failed_commit_emits_nothing() {
    let fakes = Fakes::failing_transition();

    assert_eq!(
        fakes.context().save_changed(&fakes.runtime, &fakes.operation).await,
        Err(RuntimeTransitionRepositoryError::Unavailable),
    );
    assert!(fakes.events.events().is_empty());
}
```

`FakeTransitionRepository` sets a shared `AtomicBool` only when its simulated commit succeeds. `RecordingEventSink` records that flag alongside each event so `all_emitted_after_commit()` is a real ordering assertion. `FakeWorkspaceRepository` returns either the attached or detached `Workspace` projection.

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::transition::tests::attached_transition_commits_before_ordered_events -- --exact
```

Expected: compilation fails because the generic port and transition context do not exist.

- [ ] **Step 3: Add the generic transition write port**

Create `application/runtimes/ports/runtime_transition_repository.rs`:

```rust
use crate::application::{
    lifecycle::LifecycleOperation,
    runtimes::RuntimeModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeTransitionRepositoryError {
    #[error("runtime already exists")]
    AlreadyExists,
    #[error("runtime operation is already running")]
    OperationAlreadyRunning,
    #[error("runtime was not found")]
    NotFound,
    #[error("runtime transition persistence is unavailable")]
    Unavailable,
    #[error("runtime transition persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeTransitionRepository<R: RuntimeModel>: Send + Sync {
    async fn save_transition(
        &self,
        runtime: &R,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError>;
}
```

Create `application/runtimes/ports/mod.rs` and export it from `runtimes/mod.rs`:

```rust
mod runtime_transition_repository;

pub use runtime_transition_repository::{
    RuntimeTransitionRepository, RuntimeTransitionRepositoryError,
};
```

- [ ] **Step 4: Implement the generic transition context**

Use a repository type parameter so a provider-specific repository trait can add reads without duplicating write/event code:

```rust
use std::sync::Arc;

use crate::application::{
    events::{ApplicationEvent, ApplicationEventSink},
    lifecycle::LifecycleOperation,
    runtimes::{
        ports::{RuntimeTransitionRepository, RuntimeTransitionRepositoryError},
        RuntimeModel,
    },
    workspace::ports::WorkspaceRepository,
};

#[derive(Clone)]
pub struct RuntimeTransitionContext<R, P>
where
    R: RuntimeModel,
    P: RuntimeTransitionRepository<R> + ?Sized,
{
    transitions: Arc<P>,
    workspaces: Arc<dyn WorkspaceRepository>,
    events: Arc<dyn ApplicationEventSink>,
    runtime: std::marker::PhantomData<R>,
}

impl<R, P> RuntimeTransitionContext<R, P>
where
    R: RuntimeModel,
    P: RuntimeTransitionRepository<R> + ?Sized,
{
    pub fn new(
        transitions: Arc<P>,
        workspaces: Arc<dyn WorkspaceRepository>,
        events: Arc<dyn ApplicationEventSink>,
    ) -> Self {
        Self {
            transitions,
            workspaces,
            events,
            runtime: std::marker::PhantomData,
        }
    }

    pub fn transitions(&self) -> &P {
        self.transitions.as_ref()
    }

    pub async fn save_changed(
        &self,
        runtime: &R,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        self.transitions.save_transition(runtime, operation).await?;
        self.events
            .emit(ApplicationEvent::RuntimeChanged(runtime.clone().into_runtime()));
        self.events
            .emit(ApplicationEvent::LifecycleOperationChanged(operation.clone()));
        Ok(())
    }

    pub async fn save_attached(
        &self,
        runtime: &R,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        self.transitions.save_transition(runtime, operation).await?;
        self.emit_workspace_projection(runtime.workspace_id()).await;
        self.events
            .emit(ApplicationEvent::RuntimeChanged(runtime.clone().into_runtime()));
        self.events
            .emit(ApplicationEvent::LifecycleOperationChanged(operation.clone()));
        Ok(())
    }

    pub async fn save_deleted(
        &self,
        runtime: &R,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        self.transitions.save_transition(runtime, operation).await?;
        self.emit_workspace_projection(runtime.workspace_id()).await;
        self.events.emit(ApplicationEvent::RuntimeDeleted {
            workspace_id: runtime.workspace_id().to_owned(),
            kind: runtime.kind(),
        });
        self.events
            .emit(ApplicationEvent::LifecycleOperationChanged(operation.clone()));
        Ok(())
    }

    async fn emit_workspace_projection(&self, workspace_id: &str) {
        if let Ok(Some(workspace)) = self.workspaces.get(workspace_id).await {
            self.events.emit(ApplicationEvent::WorkspaceChanged(workspace));
        }
    }
}
```

Workspace projection lookup is best-effort notification preparation after the authoritative transition commit. Its failure does not turn a committed transition into an application failure; runtime and lifecycle events are still emitted.

Export `RuntimeTransitionContext` from `application/runtimes/mod.rs`.

- [ ] **Step 5: Split RunPod reads from generic transition writes**

Change `RunpodRuntimeRepositoryError` to read-only failures:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeRepositoryError {
    #[error("runtime repository is unavailable")]
    Unavailable,
    #[error("runtime repository contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeRepository:
    RuntimeTransitionRepository<RunpodRuntime> + Send + Sync
{
    async fn get(
        &self,
        workspace_id: &str,
    ) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError>;
}
```

Update `RunpodRuntimeError` mappings:

```rust
impl From<RuntimeTransitionRepositoryError> for RunpodRuntimeError {
    fn from(error: RuntimeTransitionRepositoryError) -> Self {
        match error {
            RuntimeTransitionRepositoryError::AlreadyExists => Self::AlreadyProvisioned,
            RuntimeTransitionRepositoryError::OperationAlreadyRunning => Self::OperationInProgress,
            RuntimeTransitionRepositoryError::NotFound
            | RuntimeTransitionRepositoryError::Unavailable
            | RuntimeTransitionRepositoryError::CorruptData => Self::PersistenceUnavailable,
        }
    }
}
```

Keep the existing `From<RunpodRuntimeRepositoryError>` mapping to `PersistenceUnavailable`.

- [ ] **Step 6: Adapt SQLite writes without changing transaction behavior**

In `adapters/sqlite/runpod_runtime_repository.rs`:

- keep `get` in `impl RunpodRuntimeRepository`;
- move `save_transition` unchanged into `impl RuntimeTransitionRepository<RunpodRuntime>`;
- change write helper return types and write-error mappings from `RunpodRuntimeRepositoryError` to `RuntimeTransitionRepositoryError`;
- keep read mapping functions on `RunpodRuntimeRepositoryError`;
- preserve the single SeaORM transaction and every existing insert/update/delete case.

Split the current implementation mechanically. Keep the complete current `get`
method under `impl RunpodRuntimeRepository for
SqliteRunpodRuntimeRepository`. Move the complete current `save_transition`
method under `impl RuntimeTransitionRepository<RunpodRuntime> for
SqliteRunpodRuntimeRepository`; its return type becomes
`Result<(), RuntimeTransitionRepositoryError>`. Do not leave either method as a
declaration-only signature and do not alter the transaction branches while
moving them.

- [ ] **Step 7: Run GREEN and compile the adapter**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::transition::tests -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all four transition-context tests pass; the SQLite adapter compiles; formatting exits 0.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/application/runtimes src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs
git commit -m "feat(application): publish committed runtime transitions"
```

---

### Task 3: Workspace Create and Delete Events

**Files:**
- Modify: `src-tauri/src/application/workspace/service.rs`

**Interfaces:**
- Consumes: Task 1 `ApplicationEventSink` and existing workspace repository behavior.
- Produces: `WorkspaceService` create/delete event semantics with no events for rejected writes.

- [ ] **Step 1: Add failing workspace event tests**

Extend the existing local fakes with a recording sink and construct `WorkspaceService` with it. Add:

```rust
#[tokio::test]
async fn create_emits_the_committed_workspace() {
    let fakes = Fakes::with_workflow();

    let workspace = fakes
        .service()
        .create("workspace-1", CatalogRef::new("workflow", "1.0.0"))
        .await
        .unwrap();

    assert_eq!(
        fakes.events.events(),
        vec![ApplicationEvent::WorkspaceChanged(workspace)],
    );
}

#[tokio::test]
async fn delete_emits_only_after_the_workspace_is_removed() {
    let fakes = Fakes::with_unprovisioned_workspace();

    fakes.service().delete("workspace-1").await.unwrap();

    assert!(!fakes.workspaces.contains("workspace-1"));
    assert_eq!(
        fakes.events.events(),
        vec![ApplicationEvent::WorkspaceDeleted {
            workspace_id: "workspace-1".into(),
        }],
    );
}
```

Also add `assert!(fakes.events.events().is_empty())` to the existing unknown-workflow, attached-runtime, and running-operation rejection tests.

Update the local fake shapes explicitly:

```rust
#[derive(Default)]
struct RecordingApplicationEventSink(Mutex<Vec<ApplicationEvent>>);

impl RecordingApplicationEventSink {
    fn events(&self) -> Vec<ApplicationEvent> {
        self.0.lock().unwrap().clone()
    }
}

impl ApplicationEventSink for RecordingApplicationEventSink {
    fn emit(&self, event: ApplicationEvent) {
        self.0.lock().unwrap().push(event);
    }
}

struct FakeWorkflowCatalog {
    gets: Mutex<Vec<CatalogRef>>,
    workflow: Option<WorkflowDefinition>,
}

async fn get(
    &self,
    id: &str,
    revision: &str,
) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError> {
    self.gets.lock().unwrap().push(CatalogRef::new(id, revision));
    Ok(self.workflow.clone().filter(|workflow| {
        workflow.summary.id == id && workflow.summary.revision == revision
    }))
}
```

Add `events: RecordingApplicationEventSink` to `Fakes`. Define
`Fakes::with_workflow()` with an empty workspace list and `Some(workflow())`;
define `Fakes::with_unprovisioned_workspace()` with the existing unprovisioned
workspace and no operations. `with_missing_workflow()` keeps `workflow: None`.
Use this exact successful catalog value:

```rust
fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        summary: WorkflowSummary {
            id: "workflow".into(),
            revision: "1.0.0".into(),
            name: "Workflow".into(),
            description: "Workflow description".into(),
            required_volume_size_gb: 1,
            requires_hugging_face_api_key: false,
        },
        runtime_preset_ref: CatalogRef::new("runpod-preset", "1.0.0"),
        contract_requirements: vec![RuntimeContractRequirements::Runpod(
            RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
            },
        )],
        model_assets: serde_json::json!([]),
        execution_contract: serde_json::json!({}),
        workflow_graph: serde_json::json!({}),
    }
}
```

`Fakes::service()` passes `&self.events` as the fourth
`WorkspaceService::new` argument.

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::workspace::service::tests::create_emits_the_committed_workspace -- --exact
```

Expected: compilation fails because `WorkspaceService` has no event sink and emits nothing.

- [ ] **Step 3: Inject and emit application events**

Add `events` to the service:

```rust
pub struct WorkspaceService<'a> {
    workspaces: &'a dyn WorkspaceRepository,
    lifecycle: &'a dyn LifecycleOperationRepository,
    workflows: &'a dyn WorkflowCatalog,
    events: &'a dyn ApplicationEventSink,
}

pub fn new(
    workspaces: &'a dyn WorkspaceRepository,
    lifecycle: &'a dyn LifecycleOperationRepository,
    workflows: &'a dyn WorkflowCatalog,
    events: &'a dyn ApplicationEventSink,
) -> Self
```

After successful create:

```rust
let workspace = self
    .workspaces
    .create(workspace)
    .await
    .map_err(|error| match error {
        WorkspaceRepositoryError::AlreadyExists => WorkspaceError::AlreadyExists,
        WorkspaceRepositoryError::Unavailable | WorkspaceRepositoryError::CorruptData => {
            WorkspaceError::PersistenceUnavailable
        }
    })?;
self.events
    .emit(ApplicationEvent::WorkspaceChanged(workspace.clone()));
Ok(workspace)
```

After repository delete returns `true`:

```rust
self.events.emit(ApplicationEvent::WorkspaceDeleted {
    workspace_id: id.to_owned(),
});
Ok(())
```

Do not emit before repository success and do not emit from `get` or `list`.

- [ ] **Step 4: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::workspace::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all workspace service tests pass with exact create/delete/rejection event assertions.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/application/workspace/service.rs
git commit -m "feat(workspace): publish workspace events"
```

---

### Task 4: Arc-Owned RunPod Service

**Files:**
- Modify: `src-tauri/src/application/runtimes/runpod/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: Tokio runtime and Task 2 `RuntimeTransitionContext`.
- Produces: cloneable Arc-owned `RunpodRuntimeService` ready for detached entry methods.

- [ ] **Step 1: Enable explicit Tokio sync support**

In `Cargo.toml` change Tokio features to:

```toml
tokio = { version = "1", features = ["fs", "macros", "rt-multi-thread", "sync", "time"] }
```

- [ ] **Step 2: Convert `RunpodRuntimeService` dependencies to Arc**

Replace borrowed public fields with private Arc-owned dependencies and a constructor dependency struct:

```rust
#[derive(Clone)]
pub struct RunpodRuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    runtimes: Arc<dyn RunpodRuntimeRepository>,
    runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    lifecycle: Arc<dyn LifecycleOperationRepository>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn RunpodRuntimeProvider>,
    transitions: RuntimeTransitionContext<RunpodRuntime, dyn RunpodRuntimeRepository>,
}

pub struct RunpodRuntimeServiceDependencies {
    pub workspaces: Arc<dyn WorkspaceRepository>,
    pub workflows: Arc<dyn WorkflowCatalog>,
    pub runtimes: Arc<dyn RunpodRuntimeRepository>,
    pub runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    pub lifecycle: Arc<dyn LifecycleOperationRepository>,
    pub secrets: Arc<dyn SecretStore>,
    pub provider: Arc<dyn RunpodRuntimeProvider>,
    pub events: Arc<dyn ApplicationEventSink>,
}
```

`RunpodRuntimeService::new` constructs `RuntimeTransitionContext` from cloned runtime/workspace/event dependencies.

```rust
impl RunpodRuntimeService {
    pub fn new(dependencies: RunpodRuntimeServiceDependencies) -> Self {
        let transitions = RuntimeTransitionContext::new(
            dependencies.runtimes.clone(),
            dependencies.workspaces.clone(),
            dependencies.events,
        );
        Self {
            workspaces: dependencies.workspaces,
            workflows: dependencies.workflows,
            runtimes: dependencies.runtimes,
            runtime_catalog: dependencies.runtime_catalog,
            lifecycle: dependencies.lifecycle,
            secrets: dependencies.secrets,
            provider: dependencies.provider,
            transitions,
        }
    }
}
```

Export `RunpodRuntimeServiceDependencies` beside the existing service types from
`application/runtimes/runpod/mod.rs`.

Keep current synchronous `provision`, `cleanup`, `fail_interrupted`, and direct
transition-write behavior unchanged in this task. Task 5 and Task 6 change the
entry methods after the ownership refactor is independently green.

Update `test_support.rs` so fakes are held in `Arc`, `service()` returns an owned `RunpodRuntimeService`, and assertions inspect the same shared fake instances. Do not add a second constructor or compatibility wrapper.

Add the shared RunPod test sink required by the service constructor:

```rust
#[derive(Default)]
pub(super) struct RecordingApplicationEventSink {
    events: Mutex<Vec<ApplicationEvent>>,
    changed: tokio::sync::Notify,
}

impl ApplicationEventSink for RecordingApplicationEventSink {
    fn emit(&self, event: ApplicationEvent) {
        self.events.lock().unwrap().push(event);
        self.changed.notify_waiters();
    }
}

impl RecordingApplicationEventSink {
    pub fn events(&self) -> Vec<ApplicationEvent> {
        self.events.lock().unwrap().clone()
    }
}
```

Store it as `Arc<RecordingApplicationEventSink>` in the RunPod fakes and pass a
clone through `RunpodRuntimeServiceDependencies.events`.

- [ ] **Step 3: Run the existing service suite**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all existing RunPod service tests pass after Arc ownership conversion.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/application/runtimes/runpod/mod.rs src-tauri/src/application/runtimes/runpod/service.rs src-tauri/src/application/runtimes/runpod/test_support.rs
git commit -m "refactor(runpod): prepare detached lifecycle execution"
```

---

### Task 5: Detached Provision Flow With Events on Every Transition

**Files:**
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`

**Interfaces:**
- Consumes: Task 2 transition context and Task 4 Arc-owned service.
- Produces: `RunpodRuntimeService::start_provision(...) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>` with detached six-step execution.

- [ ] **Step 1: Add a deterministic provider gate and the failing detached-start test**

Extend `FakeRunpodRuntimeProvider` with two `tokio::sync::Notify` values and an
`AtomicBool` gate. Make its shared call recorder async so the gate works for the
first provision or cleanup provider method:

```rust
async fn call(&self, method: &'static str) -> Result<(), RunpodRuntimeProviderError> {
    self.calls.lock().unwrap().push(method);
    if self.block_first_call.swap(false, Ordering::SeqCst) {
        self.entered.notify_one();
        self.release.notified().await;
    }
    let mut fail_once = self.fail_once.lock().unwrap();
    if *fail_once == Some(method) {
        *fail_once = None;
        Err(RunpodRuntimeProviderError::Unavailable)
    } else {
        Ok(())
    }
}
```

Every fake provider method calls `self.call("method_name").await` before
returning its configured value.

Add helpers:

```rust
pub fn block_first_call(&self) {
    self.block_first_call.store(true, Ordering::SeqCst);
}

pub async fn wait_until_first_call(&self) {
    self.entered.notified().await;
}

pub fn release_first_call(&self) {
    self.release.notify_one();
}
```

Add one shared command helper so every detached provision test uses identical
input:

```rust
pub fn provision_command() -> ProvisionRunpodRuntime {
    ProvisionRunpodRuntime {
        workspace_id: "workspace-1".into(),
        datacenter_id: "dc-1".into(),
        gpu_id: "gpu-1".into(),
        volume_size_gb: 19,
    }
}
```

Add the test:

```rust
#[tokio::test]
async fn start_provision_returns_a_durable_operation_before_provider_work_finishes() {
    let fakes = ProvisionFakes::ready();
    fakes.provider.block_first_call();

    let (runtime, operation) = fakes
        .service()
        .start_provision(provision_command())
        .await
        .unwrap();

    assert_eq!(runtime.state, RunpodRuntimeState::Provisioning);
    assert_eq!(operation.state, LifecycleOperationState::Running);
    assert_eq!(operation.progress.provision_step(), Some(RunpodProvisionStep::CreateNetworkVolume));
    assert_eq!(fakes.repository.last_snapshot(), (runtime.clone(), operation.clone()));
    assert_eq!(fakes.events.events(), vec![
        ApplicationEvent::WorkspaceChanged(fakes.workspace_snapshot()),
        ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime.clone())),
        ApplicationEvent::LifecycleOperationChanged(operation.clone()),
    ]);

    fakes.provider.wait_until_first_call().await;
    assert_eq!(fakes.repository.last_operation_state(), LifecycleOperationState::Running);

    fakes.provider.release_first_call();
    fakes.events.wait_for_terminal_operation(operation.id).await;
    assert_eq!(fakes.repository.last_operation_state(), LifecycleOperationState::Succeeded);
}
```

Extend the shared RunPod test sink from Task 4 with deterministic terminal waits
and event counters:

```rust
impl RecordingApplicationEventSink {
    pub async fn wait_for_terminal_operation(&self, id: Uuid) {
        loop {
            let changed = self.changed.notified();
            if self.events.lock().unwrap().iter().any(|event| {
                matches!(
                    event,
                    ApplicationEvent::LifecycleOperationChanged(operation)
                        if operation.id == id
                            && operation.state != LifecycleOperationState::Running
                )
            }) {
                return;
            }
            changed.await;
        }
    }

    pub fn runtime_changed_count(&self) -> usize {
        self.events.lock().unwrap().iter().filter(|event| {
            matches!(event, ApplicationEvent::RuntimeChanged(_))
        }).count()
    }

    pub fn runtime_deleted_count(&self) -> usize {
        self.events.lock().unwrap().iter().filter(|event| {
            matches!(event, ApplicationEvent::RuntimeDeleted { .. })
        }).count()
    }

    pub fn lifecycle_event_count(&self) -> usize {
        self.events.lock().unwrap().iter().filter(|event| {
            matches!(event, ApplicationEvent::LifecycleOperationChanged(_))
        }).count()
    }

    pub fn workspace_event_count(&self) -> usize {
        self.events.lock().unwrap().iter().filter(|event| {
            matches!(event, ApplicationEvent::WorkspaceChanged(_))
        }).count()
    }
}
```

Every wait registers its `Notify` future before checking the recorded state, so
an event cannot be lost between the check and await. No test uses a sleep.

During the Task 4 fake ownership rewrite, back `FakeWorkspaceRepository` with a
shared `Arc<Mutex<Vec<Workspace>>>` also held by
`FakeRunpodRuntimeRepository`. Its transition write mirrors the SQLite anchor
projection: inserting the first RunPod runtime sets `attached_runtime` to
`Some(RuntimeKind::Runpod)`, and successful cleanup deletion sets it to `None`.
This makes `save_attached`/`save_deleted` workspace events authoritative in
application tests rather than hard-coded sink payloads.

Keep that shared value on `ProvisionFakes`/`CleanupFakes` as
`workspace_rows: Arc<Mutex<Vec<Workspace>>>` and expose the committed projection
through:

```rust
pub fn workspace_snapshot(&self) -> Workspace {
    self.workspace_rows
        .lock()
        .unwrap()
        .iter()
        .find(|workspace| workspace.id == "workspace-1")
        .cloned()
        .expect("workspace fixture should exist")
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::start_provision_returns_a_durable_operation_before_provider_work_finishes -- --exact
```

Expected: compilation fails because `start_provision`, provider gates, and recording events do not exist.

- [ ] **Step 3: Split provision start from detached execution**

Rename the public entry method and change its return type:

```rust
pub async fn start_provision(
    &self,
    command: ProvisionRunpodRuntime,
) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>
```

Keep all existing preflight before runtime/operation construction. After constructing them:

```rust
self.transitions.save_attached(&runtime, &operation).await?;

let initial_runtime = runtime.clone();
let initial_operation = operation.clone();
let service = self.clone();
tokio::spawn(async move {
    service
        .run_provision(command, definition, workflow, runpod_key, hugging_face_api_key, runtime, operation)
        .await;
});

Ok((initial_runtime, initial_operation))
```

Move the six provider steps into private `run_provision`. It returns `()` and handles every provider error by calling `fail_transition`; if failure persistence itself fails, it returns immediately without further provider calls.

Replace direct repository writes in `set_provision_step`, `fail_transition`, and terminal success with `self.transitions.save_changed(...)`.

No provider call may remain in `start_provision` before `spawn`.

- [ ] **Step 4: Convert existing provision tests to detached terminal assertions**

The happy path waits for the operation ID returned by `start_provision`, then asserts:

```rust
assert_eq!(fakes.repository.running_steps(), vec![
    RunpodProvisionStep::CreateNetworkVolume,
    RunpodProvisionStep::StartProvisionerPod,
    RunpodProvisionStep::PollProvisioner,
    RunpodProvisionStep::TerminateProvisionerPod,
    RunpodProvisionStep::CreateTemplate,
    RunpodProvisionStep::CreateEndpoint,
]);
assert_eq!(fakes.events.runtime_changed_count(), 7);
assert_eq!(fakes.events.lifecycle_event_count(), 7);
assert_eq!(fakes.events.workspace_event_count(), 1);
```

The provider-failure table starts provision successfully, waits for terminal failure, and retains the existing per-step resource/failing-progress assertions. It additionally asserts that the last two events are the failed `RuntimeChanged` and failed `LifecycleOperationChanged` snapshots.

Add a preflight test proving a missing credential produces no saved snapshot, provider call, event, or spawned work.

Add a transition-persistence failure test. Configure the fake repository to
fail the transition after the initial commit, start provision, release the
provider gate, and wait until the attempted write is observed. Assert that no
later provider method is called, the last durable operation remains the initial
`Running/CreateNetworkVolume` snapshot, and no terminal event is emitted.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::start_provision_returns_a_durable_operation_before_provider_work_finishes -- --exact
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::provider_failures_persist_the_failing_step_and_created_resources -- --exact
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: detached start, exact progress events, happy path, every provider failure, and preflight-without-events tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/runtimes/runpod/service.rs src-tauri/src/application/runtimes/runpod/test_support.rs
git commit -m "feat(runpod): provision in background with events"
```

---

### Task 6: Detached Cleanup, Runtime Deletion Events, and Recovery Events

**Files:**
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`

**Interfaces:**
- Consumes: Tasks 2/4 generic transition and Arc-owned service facilities.
- Produces: `start_cleanup(...) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>` plus event-producing interrupted recovery.

- [ ] **Step 1: Write the failing detached cleanup test**

Add:

```rust
#[tokio::test]
async fn start_cleanup_returns_running_snapshots_and_finishes_in_background() {
    let fakes = CleanupFakes::ready_runtime();
    fakes.provider.block_first_call();

    let (runtime, operation) = fakes
        .service()
        .start_cleanup("workspace-1")
        .await
        .unwrap();

    assert_eq!(runtime.state, RunpodRuntimeState::CleaningUp);
    assert_eq!(operation.state, LifecycleOperationState::Running);
    assert_eq!(operation.progress.cleanup_step(), Some(RunpodCleanupStep::DeleteEndpoint));
    assert_eq!(fakes.events.events(), vec![
        ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime.clone())),
        ApplicationEvent::LifecycleOperationChanged(operation.clone()),
    ]);

    fakes.provider.wait_until_first_call().await;
    assert!(!fakes.repository.runtime_was_removed());

    fakes.provider.release_first_call();
    fakes.events.wait_for_terminal_operation(operation.id).await;
    assert!(fakes.repository.runtime_was_removed());
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::start_cleanup_returns_running_snapshots_and_finishes_in_background -- --exact
```

Expected: compilation fails because cleanup is still awaited and returns `()`.

- [ ] **Step 3: Split cleanup start from detached execution**

Change the entry method to:

```rust
pub async fn start_cleanup(
    &self,
    workspace_id: &str,
) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError>
```

Preflight still loads runtime and RunPod credential before mutation. After `begin_cleanup` and operation creation:

```rust
self.transitions.save_changed(&runtime, &operation).await?;

let initial_runtime = runtime.clone();
let initial_operation = operation.clone();
let service = self.clone();
let workspace_id = workspace_id.to_owned();
tokio::spawn(async move {
    service
        .run_cleanup(workspace_id, runpod_key, runtime, operation)
        .await;
});

Ok((initial_runtime, initial_operation))
```

Move provider deletion steps into private `run_cleanup`. Ordinary steps and failures use `save_changed`. Terminal success calls `operation.succeed(...)` and then `save_deleted`.

- [ ] **Step 4: Convert cleanup and recovery tests to event-aware detached assertions**

Update existing cleanup tests to wait for the returned operation ID before asserting provider calls/resources.

For successful cleanup, assert exact totals:

```rust
assert_eq!(fakes.events.runtime_changed_count(), 4);
assert_eq!(fakes.events.runtime_deleted_count(), 1);
assert_eq!(fakes.events.lifecycle_event_count(), 5);
assert_eq!(fakes.events.workspace_event_count(), 1);
```

Assert the final three events are:

```rust
ApplicationEvent::WorkspaceChanged(detached_workspace),
ApplicationEvent::RuntimeDeleted {
    workspace_id: "workspace-1".into(),
    kind: RuntimeKind::Runpod,
},
ApplicationEvent::LifecycleOperationChanged(succeeded_operation),
```

Provider failure waits for terminal failure and asserts the last two events are failed runtime and lifecycle snapshots. Missing runtime/credential preflight tests assert no events and no provider calls.

Update `startup_marks_running_operations_and_runtimes_failed` to assert two `RuntimeChanged(Failed)` and two `LifecycleOperationChanged(Failed)` events, with original trace IDs and progress retained. Recovery emits no workspace event and makes no provider call.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::start_cleanup_returns_running_snapshots_and_finishes_in_background -- --exact
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::startup_marks_running_operations_and_runtimes_failed -- --exact
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: provision remains green; cleanup starts detached; every cleanup/recovery transition emits the approved events in order.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/runtimes/runpod/service.rs src-tauri/src/application/runtimes/runpod/test_support.rs
git commit -m "feat(runpod): cleanup in background with events"
```

---

### Task 7: Full Native Verification and Scope Audit

**Files:**
- No changes expected. Return failures to the task that owns the failing file.

**Interfaces:**
- Consumes: Tasks 1-6.
- Produces: verified provider-neutral background/events application layer with no inbound-adapter changes.

- [ ] **Step 1: Run the complete application suite**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application:: -- --nocapture
```

Expected: all application tests pass; detached tests terminate through synchronization signals and leave no blocked tasks.

- [ ] **Step 2: Run native verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all tests, formatting, and strict Clippy pass.

- [ ] **Step 3: Verify dependency and scope boundaries**

```bash
rg -n "tauri|specta|serde::|#\[serde|crate::(infra|adapters)" src-tauri/src/application
rg -n "ApplicationEvent|ApplicationEventSink" src-tauri/src/adapters src-tauri/src/infra
rg -n "tokio::spawn" src-tauri/src/application/runtimes
git diff --name-only 0ca01b98..HEAD -- src src/generated/commands.ts
```

Expected:

- application has no Tauri/Specta/serde facade or infra/adapter dependency;
- adapters/infra contain no concrete event sink or event mapping;
- production `tokio::spawn` calls appear only in the RunPod service detached entry methods; transition tests may use it for deterministic concurrent futures;
- no frontend or generated command file changed.

- [ ] **Step 4: Verify background and event requirements mechanically**

```bash
rg -n "save_(changed|attached|deleted)" src-tauri/src/application/runtimes/runpod/service.rs
rg -n "ApplicationEvent::" src-tauri/src/application
rg -n "sleep\(" src-tauri/src/application/runtimes
```

Expected:

- RunPod flow uses the generic transition context for initial, progress, failure, terminal, deletion, and recovery writes;
- application events are emitted only by workspace service and transition context;
- detached tests add no timing sleeps. The existing production provider polling sleep is outside these application paths and is unaffected.

- [ ] **Step 5: Confirm explicitly skipped surfaces**

Do not run or repair:

```text
bun run codegen:commands
bun run build
bun run lint
Tauri runtime
frontend runtime
```

Confirm `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, and all files under `src/` remain unchanged.

---

## Spec Coverage

- Provider-neutral `Runtime`/`RuntimeModel`: Task 1.
- Application model events and sink port: Task 1.
- Commit-before-event ordering and attach/detach semantics: Task 2.
- Generic persistence contract reusable by future providers: Task 2.
- Workspace create/delete events: Task 3.
- Arc-owned service dependencies suitable for `'static` futures: Task 4.
- Direct detached Tokio execution after durable starts: Tasks 5-6.
- Durable initial provision transition, immediate snapshots, detached provider work: Task 5.
- Event on every provision transition and terminal failure/success: Task 5.
- Durable initial cleanup transition, detached cleanup, runtime deletion events: Task 6.
- Recovery events without provider calls: Task 6.
- No Tauri/inbound/frontend/concrete sink, retry/resume/reconciliation/task registry: Tasks 1-7 constraints and Task 7 audit.

No approved design requirement is intentionally omitted.
