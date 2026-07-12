# Tauri Facade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first active Tauri facade with provider-dispatched runtime persistence, generated command/event contracts, hydrated workspaces, paginated reads, runtime workflows, and fatal native wiring.

**Architecture:** `Workspace` owns `Option<Runtime>`, while `workspace_runtimes` stores the provider-neutral runtime kind and shared lifecycle state. Closed facade and SQLite dispatchers contain exhaustive routing only; all RunPod workflow, SQL, mapping, resources, and progress behavior remains in RunPod-specific modules. Generic SQLite transition orchestration owns one transaction across the anchor, operation, provider extension, and provider progress.

**Tech Stack:** Rust 2021, Tauri 2, Tauri Specta, Specta, Serde, SeaORM 2 RC, Tokio, fastrace diagnostics, TypeScript, Bun.

## Global Constraints

- Follow root `AGENTS.md` and `src-tauri/AGENTS.md`.
- Keep Tauri, Specta, Serde transport derives, and frontend DTOs out of `application/**`.
- Keep raw API keys and provider resource IDs out of facade outputs, events, errors, logs, fixtures, and generated TypeScript.
- Every Tauri command is `#[diagnostic(root)]`; API-key request fields are `#[diagnostic(redact)]`.
- `Workspace::runtime` is `Option<Runtime>`; multiple runtimes are out of scope.
- `RuntimeState` is exactly `Provisioning`, `Ready`, `CleaningUp`, or `Failed` for every provider.
- Keep `workspace_runtimes` as the one-runtime anchor with `runtime_kind` and shared `state`.
- Store `runtime_kind` on every `runtime_operations` row so provider progress remains resolvable after cleanup.
- Generic runtime files may mention RunPod only in exhaustive dispatch arms. RunPod SQL, field mapping, lifecycle branches, resources, and progress mapping live in provider-specific modules.
- Use closed dispatchers; add no dispatcher trait, dynamic registry, factory, or plugin loading.
- Keep generic anchor, operation, provider extension, and provider progress writes in one SQLite transaction.
- Use `offset`, `limit`, and `total`; validate `limit` as `1..=100`.
- Emit `workspace_changed` before `runtime_operation` after each durable runtime transition.
- Keep events best-effort; add no relay, cache, revision, retry, or outbox.
- Use `app_data_dir/db.sqlite` and `app_data_dir/diagnostics.log` and update `src-tauri/AGENTS.md`, root `README.md`, and `src-tauri/README.md` to match.
- Keep RunPod network volume size bounded at 4,000 GB.
- Do not manually edit `src/generated/commands.ts`; regenerate it with `bun run codegen:commands`.

---

## File Structure

### Application and provider persistence

- `src-tauri/src/application/runtimes/model.rs`: provider-neutral runtime, state, kind, operation, and progress envelopes.
- `src-tauri/src/application/workspace/model.rs`: workspace aggregate with optional complete runtime.
- `src-tauri/src/application/runtimes/transition.rs`: commit followed by ordered complete events.
- `src-tauri/src/application/runtimes/ports/runtime_transition_repository.rs`: aggregate transition port.
- `src-tauri/src/application/runtimes/runpod/model.rs`: RunPod configuration, resources, placement, and progress only.
- `src-tauri/src/application/runtimes/runpod/service.rs`: RunPod provision, cleanup, placement, and recovery workflows.
- `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs`: provider-neutral runtime anchor.
- `src-tauri/src/infra/sqlite/entities/runtime_operations.rs`: provider-neutral operation row with runtime discriminator.
- `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`: RunPod extension without shared state.
- `src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs`: exhaustive provider routing only.
- `src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs`: all RunPod SQLite access and mapping.
- `src-tauri/src/adapters/sqlite/runtime_transition_repository.rs`: generic atomic anchor/operation orchestration.
- `src-tauri/src/adapters/sqlite/workspace_repository.rs`: hydrated workspace reads through persistence dispatch.
- `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs`: operation reads with dispatched progress hydration.

### Facade and composition root

- `src-tauri/src/facade/mod.rs`: Tauri Specta builder and binding export.
- `src-tauri/src/facade/model.rs`: request, response, and event DTOs and conversions.
- `src-tauri/src/facade/errors.rs`: shared envelope and command-specific code enums.
- `src-tauri/src/facade/state.rs`: `FacadeState`, pagination validation, and concrete facade runtime dispatcher.
- `src-tauri/src/facade/commands.rs`: thin root Tauri commands.
- `src-tauri/src/facade/events.rs`: synchronous `TauriEventSink`.
- `src-tauri/src/diagnostics/mod.rs`: initialize an exact log path.
- `src-tauri/src/lib.rs`: fatal bootstrap, adapters, services, recovery, and managed state.
- `src-tauri/src/main.rs`: call `luma_forge_lib::run()`.
- `src-tauri/tauri.conf.json`: package `../bundled/` as `bundled/`.
- `src/pages/home/ui/home-page.tsx`: compile-only update of the existing probe.
- Generated by tooling: `src/generated/commands.ts`.

---

### Task 1: Add Provider-Dispatched Runtime Persistence

**Files:**
- Modify: `src-tauri/src/application/runtimes/model.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs`
- Modify: `src-tauri/src/application/workspace/model.rs`
- Modify: `src-tauri/src/application/events.rs`
- Modify: `src-tauri/src/application/runtimes/transition.rs`
- Modify: `src-tauri/src/application/runtimes/ports/runtime_transition_repository.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/model.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/errors.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`
- Delete: `src-tauri/src/application/runtimes/runpod/ports/runtime_repository.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/ports/mod.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/runtime_operations.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`
- Rename: `src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs` to `src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs`
- Create: `src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs`
- Create: `src-tauri/src/adapters/sqlite/runtime_transition_repository.rs`
- Modify: `src-tauri/src/adapters/sqlite/workspace_repository.rs`
- Modify: `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs`
- Modify: `src-tauri/src/adapters/sqlite/mod.rs`
- Create test: `src-tauri/tests/sqlite_runtime_dispatch.rs`

**Interfaces:**
- Consumes: existing RunPod workflows, SQLite entities, `WorkspaceRepository`, `RuntimeOperationRepository`, and `ApplicationEventSink`.
- Produces: `Runtime { state, provider }`, `RuntimeOperation` with a persisted `runtime_kind`, `RuntimeTransitionRepository::save_transition(&Workspace, &RuntimeOperation)`, and closed SQLite provider dispatch.

- [ ] **Step 1: Write failing application model and event tests**

Replace the RunPod-specific runtime-state fixtures with tests for the shared state and complete events:

```rust
#[test]
fn runtime_kind_comes_from_its_provider() {
    let runtime = Runtime {
        state: RuntimeState::Provisioning,
        provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
            RunpodRuntimeConfig {
                datacenter_id: "EU-RO-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 100,
            },
        )),
    };

    assert_eq!(runtime.kind(), RuntimeKind::Runpod);
}
```

In `start_provision_returns_a_durable_operation_before_provider_work_finishes`, require only the complete aggregate and operation events:

```rust
assert_eq!(
    fakes.events.events()[..2],
    [
        ApplicationEvent::WorkspaceChanged(Workspace {
            runtime: Some(Runtime {
                state: RuntimeState::Provisioning,
                provider: RuntimeProvider::Runpod(runtime.clone()),
            }),
            ..fakes.workspace_snapshot()
        }),
        ApplicationEvent::RuntimeOperationChanged(operation.clone()),
    ]
);
assert_eq!(operation.runtime_kind, RuntimeKind::Runpod);
```

- [ ] **Step 2: Write failing SQLite dispatch tests**

Create `src-tauri/tests/sqlite_runtime_dispatch.rs` with four tests:

```rust
#[tokio::test]
async fn workspace_hydrates_state_and_runpod_extension_through_dispatch() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);

    fixture.transitions.save_transition(&workspace, &operation).await.unwrap();

    assert_eq!(fixture.workspaces.get(&workspace.id).await.unwrap(), Some(workspace));
    let anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection()).await.unwrap().unwrap();
    assert_eq!(anchor.runtime_kind, "runpod");
    assert_eq!(anchor.state, "provisioning");
    let extension = runpod_workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection()).await.unwrap().unwrap();
    assert_eq!(extension.volume_size_gb, 100);
}

#[tokio::test]
async fn provider_failure_rolls_back_anchor_and_operation() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, u64::MAX));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);

    assert_eq!(
        fixture.transitions.save_transition(&workspace, &operation).await,
        Err(RuntimePersistenceError::CorruptData),
    );
    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection()).await.unwrap().is_none());
    assert!(runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(fixture.database.connection()).await.unwrap().is_none());
}

#[tokio::test]
async fn cleanup_removes_runtime_but_keeps_dispatched_operation_progress() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut workspace = fixture.workspaces.get("workspace-1").await.unwrap().unwrap();
    workspace.runtime = None;
    let mut operation = running_operation("workspace-1", RuntimeOperationKind::Cleanup);
    operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

    fixture.transitions.save_transition(&workspace, &operation).await.unwrap();

    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection()).await.unwrap().is_none());
    assert!(runpod_workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection()).await.unwrap().is_none());
    let stored = fixture.operations
        .recent_for_workspace("workspace-1", 10).await.unwrap()
        .into_iter().find(|stored| stored.id == operation.id).unwrap();
    assert_eq!(stored.runtime_kind, RuntimeKind::Runpod);
    assert_eq!(stored.progress, operation.progress);
}

#[tokio::test]
async fn anchor_without_provider_extension_is_corrupt() {
    let fixture = Fixture::with_orphaned_anchor().await;
    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData),
    );
}
```

The test file defines concrete `Fixture`, `runpod_runtime`, and `running_operation` helpers using `SqliteInfraDatabase`, the three SQLite repositories, and `Uuid::new_v4()`. `running_operation` sets `runtime_kind: RuntimeKind::Runpod` and matching RunPod progress.

- [ ] **Step 3: Run the focused tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml runtime_kind_comes_from_its_provider
cargo test --manifest-path src-tauri/Cargo.toml workspace_hydrates_state_and_runpod_extension_through_dispatch
cargo test --manifest-path src-tauri/Cargo.toml provider_failure_rolls_back_anchor_and_operation
```

Expected: FAIL because shared runtime state, operation runtime kind, aggregate transition persistence, and SQLite dispatcher do not exist.

- [ ] **Step 4: Replace the application runtime aggregate**

Use these exact provider-neutral types:

```rust
#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind { Runpod }

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState { Provisioning, Ready, CleaningUp, Failed }

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub enum RuntimeProvider {
    Runpod(#[diagnostic(show)] RunpodRuntime),
}

impl RuntimeProvider {
    pub fn kind(&self) -> RuntimeKind {
        match self { Self::Runpod(_) => RuntimeKind::Runpod }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Runtime {
    #[diagnostic(show)]
    pub state: RuntimeState,
    #[diagnostic(show)]
    pub provider: RuntimeProvider,
}

impl Runtime {
    pub fn kind(&self) -> RuntimeKind { self.provider.kind() }
}
```

Add `pub runtime_kind: RuntimeKind` to `RuntimeOperation` and to `RuntimeOperation::running`. Remove `RuntimeModel`. Change `Workspace::runtime` to `Option<Runtime>`. Reduce events to:

```rust
pub enum ApplicationEvent {
    WorkspaceChanged(Workspace),
    WorkspaceDeleted { workspace_id: String },
    RuntimeOperationChanged(RuntimeOperation),
}
```

Change the transition port and context to:

```rust
#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimePersistenceError {
    #[error("runtime already exists")]
    AlreadyExists,
    #[error("runtime operation is already running")]
    OperationAlreadyRunning,
    #[error("runtime was not found")]
    NotFound,
    #[error("runtime persistence is unavailable")]
    Unavailable,
    #[error("runtime persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait RuntimeTransitionRepository: Send + Sync {
    async fn save_transition(
        &self,
        workspace: &Workspace,
        operation: &RuntimeOperation,
    ) -> Result<(), RuntimePersistenceError>;
}

pub async fn save(
    &self,
    workspace: &Workspace,
    operation: &RuntimeOperation,
) -> Result<(), RuntimePersistenceError> {
    let _guard = self.coordinator.lock().await;
    self.transitions.save_transition(workspace, operation).await?;
    self.events.emit(ApplicationEvent::WorkspaceChanged(workspace.clone()));
    self.events.emit(ApplicationEvent::RuntimeOperationChanged(operation.clone()));
    Ok(())
}
```

- [ ] **Step 5: Move shared state out of the RunPod model and update workflows**

`RunpodRuntime` contains only `config` and `resources`; delete `RunpodRuntimeState`, `workspace_id`, `RuntimeModel`, and the provider runtime read port. Its constructor is:

```rust
pub fn new_provisioning(config: RunpodRuntimeConfig) -> Self {
    Self { config, resources: RunpodRuntimeResources::default() }
}
```

RunPod service methods mutate `workspace.runtime.as_mut().state` through shared transition helpers and use these public entry points:

```rust
pub async fn start_provision(
    &self,
    command: ProvisionRunpodRuntime,
) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError>;

pub async fn start_cleanup(
    &self,
    workspace: Workspace,
) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError>;
```

Provision loads one workspace, rejects any attached runtime, installs a `Runtime` with `state: Provisioning` and a RunPod provider built from the command configuration, and creates an operation with `runtime_kind: Runpod`. Cleanup accepts only the RunPod provider, moves shared state to `CleaningUp`, and sets `workspace.runtime = None` only for the successful terminal transition. Recovery accepts `Vec<RuntimeOperation>` already routed to RunPod, loads their complete workspaces, and changes the shared state to `Failed`. Fakes use `WorkspaceRepository` as the only runtime read source.

Use this recovery signature:

```rust
pub async fn recover_interrupted(
    &self,
    operations: Vec<RuntimeOperation>,
) -> Result<(), RunpodRuntimeError>;
```

Replace `RunpodRuntimeServiceDependencies::runtimes` with:

```rust
pub transitions: Arc<dyn RuntimeTransitionRepository>,
```

Construct `RuntimeTransitionContext::new(dependencies.transitions, dependencies.events)`; the RunPod service must not depend on a provider-specific runtime repository.

- [ ] **Step 6: Change the provider-neutral schema**

Use these exact entity fields:

```rust
// workspace_runtimes.rs
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    pub runtime_kind: String,
    pub state: String,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id", on_delete = "Cascade")]
    pub workspace: HasOne<super::workspaces::Entity>,
}

// runtime_operations.rs
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workspace_id: String,
    pub runtime_kind: String,
    pub running_workspace_id: Option<String>,
    pub operation_kind: String,
    pub state: String,
    pub trace_id: Option<String>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub finished_at: Option<TimeDateTimeWithTimeZone>,
}
```

Remove `state` from `runpod_workspace_runtimes`. Keep its FK pointed at `workspace_runtimes.workspace_id` with `ON DELETE CASCADE`. Add no migration or compatibility fallback.

- [ ] **Step 7: Isolate RunPod persistence and add closed dispatch**

`runpod_runtime_persistence.rs` owns every reference to `runpod_workspace_runtimes`, `runpod_runtime_operation_progress`, RunPod resource columns, and RunPod progress-step strings. It exposes only SQLite-internal functions:

```rust
pub(super) async fn load_runtime<C: ConnectionTrait>(
    workspace_id: &str,
    connection: &C,
) -> Result<RunpodRuntime, RuntimePersistenceError>;

pub(super) async fn load_runtimes<C: ConnectionTrait>(
    workspace_ids: &[String],
    connection: &C,
) -> Result<HashMap<String, RunpodRuntime>, RuntimePersistenceError>;

pub(super) async fn save_runtime(
    workspace_id: &str,
    runtime: &RunpodRuntime,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError>;

pub(super) async fn save_progress(
    operation: &RuntimeOperation,
    progress: RunpodProgress,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError>;

pub(super) async fn load_progress<C: ConnectionTrait>(
    operation_ids: &[String],
    connection: &C,
) -> Result<HashMap<String, RunpodProgress>, RuntimeOperationRepositoryError>;
```

`runtime_persistence_dispatcher.rs` contains only exhaustive routing:

```rust
pub(super) fn runtime_kind_value(kind: RuntimeKind) -> &'static str {
    match kind { RuntimeKind::Runpod => "runpod" }
}

pub(super) fn parse_runtime_kind(
    value: &str,
) -> Result<RuntimeKind, RuntimePersistenceError> {
    match value {
        "runpod" => Ok(RuntimeKind::Runpod),
        _ => Err(RuntimePersistenceError::CorruptData),
    }
}

pub(super) async fn save_runtime(
    workspace_id: &str,
    provider: &RuntimeProvider,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match provider {
        RuntimeProvider::Runpod(runtime) =>
            runpod_runtime_persistence::save_runtime(workspace_id, runtime, transaction).await,
    }
}

pub(super) async fn save_progress(
    operation: &RuntimeOperation,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match operation.progress {
        RuntimeProgress::Runpod(progress) => {
            if operation.runtime_kind != RuntimeKind::Runpod {
                return Err(RuntimePersistenceError::CorruptData);
            }
            runpod_runtime_persistence::save_progress(operation, progress, transaction).await
        }
    }
}
```

Add equivalent `load_runtime`, grouped `load_runtimes`, and grouped `load_progress` dispatch functions. They may match `RuntimeKind::Runpod`; they must contain no RunPod SQL or field mapping. Generic anchor and operation repositories use `runtime_kind_value` and `parse_runtime_kind`; provider discriminator strings must not be parsed or formatted anywhere else.

- [ ] **Step 8: Implement generic atomic transition orchestration and hydration**

`SqliteRuntimeTransitionRepository` begins one transaction, checks workspace and runtime-kind consistency, writes the generic anchor and operation, dispatches provider writes, and commits. Its core branch is:

```rust
match &workspace.runtime {
    Some(runtime) if runtime.kind() == operation.runtime_kind => {
        upsert_anchor(&workspace.id, runtime.state, operation.runtime_kind, &transaction).await?;
        runtime_persistence_dispatcher::save_runtime(
            &workspace.id,
            &runtime.provider,
            &transaction,
        ).await?;
    }
    Some(_) => return Err(RuntimePersistenceError::CorruptData),
    None if operation.kind == RuntimeOperationKind::Cleanup
        && operation.state == RuntimeOperationState::Succeeded => {
            delete_anchor(&workspace.id, &transaction).await?;
        }
    None => return Err(RuntimePersistenceError::CorruptData),
}
save_operation(operation, &transaction).await?;
runtime_persistence_dispatcher::save_progress(operation, &transaction).await?;
transaction.commit().await.map_err(|_| RuntimePersistenceError::Unavailable)
```

For cleanup, save the terminal operation and progress before deleting the anchor if SQLite FK ordering requires it; the observable invariant is one commit containing all changes.

`SqliteWorkspaceRepository` joins only `workspaces` and `workspace_runtimes`, parses shared state generically, and calls dispatcher load functions for provider extensions. `SqliteRuntimeOperationRepository` parses generic rows including `runtime_kind`, groups operation IDs by kind, and dispatches provider progress loading. Delete all RunPod imports and step mapping from those generic repository files.

- [ ] **Step 9: Run verification and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml --test sqlite_runtime_dispatch
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
! rg -n "runpod_workspace|RunpodRuntime|RunpodProgress|network_volume_id|provisioner_pod_id|template_id|endpoint_id" \
  src-tauri/src/adapters/sqlite/workspace_repository.rs \
  src-tauri/src/adapters/sqlite/runtime_transition_repository.rs \
  src-tauri/src/adapters/sqlite/runtime_operation_repository.rs
```

Expected: tests, formatting, and Clippy PASS. The final `rg` prints no matches. Then commit:

```sh
git add src-tauri/src/application src-tauri/src/adapters/sqlite src-tauri/src/infra/sqlite src-tauri/tests/sqlite_runtime_dispatch.rs
git commit -m "refactor(runtimes): dispatch provider persistence"
```

---

### Task 2: Add UUID Creation And Offset Pagination

**Files:**
- Modify: `src-tauri/src/application/workspace/ports/workspace_repository.rs`
- Modify: `src-tauri/src/application/workspace/service.rs`
- Modify: `src-tauri/src/application/runtimes/ports/runtime_operation_repository.rs`
- Create: `src-tauri/src/application/runtimes/query.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs`
- Modify: `src-tauri/src/adapters/sqlite/workspace_repository.rs`
- Modify: `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs`
- Modify test: `src-tauri/tests/sqlite_runtime_dispatch.rs`

**Interfaces:**
- Consumes: hydrated workspaces and dispatched operation progress from Task 1.
- Produces: application page methods returning `(Vec<T>, u64)` and UUID-generating `WorkspaceService::create`.

- [ ] **Step 1: Write failing pagination and UUID tests**

```rust
#[tokio::test]
async fn create_generates_a_uuid() {
    let fakes = Fakes::with_workflow();
    let workspace = fakes.service()
        .create(CatalogRef::new("workflow", "1.0.0"))
        .await.unwrap();
    assert!(Uuid::parse_str(&workspace.id).is_ok());
}

#[tokio::test]
async fn workflow_page_returns_total_before_paging() {
    let fakes = Fakes::with_workflows(three_workflows());
    let (items, total) = fakes.service().list_workflows(1, 1).await.unwrap();
    assert_eq!(total, 3);
    assert_eq!(items.len(), 1);
}
```

Extend `sqlite_runtime_dispatch.rs` with three equal-timestamp workspaces. Attach RunPod runtimes to two of them, request offset `1`, limit `1`, and assert `total == 3`, descending ID tie-breaking, and complete hydration.

- [ ] **Step 2: Run tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml create_generates_a_uuid
cargo test --manifest-path src-tauri/Cargo.toml workflow_page_returns_total_before_paging
cargo test --manifest-path src-tauri/Cargo.toml workspace_page_is_stable_and_reports_total
```

Expected: FAIL because create accepts an ID and reads are unbounded.

- [ ] **Step 3: Add exact page interfaces**

```rust
async fn page(
    &self,
    offset: u64,
    limit: u64,
) -> Result<(Vec<Workspace>, u64), WorkspaceRepositoryError>;

async fn page(
    &self,
    workspace_id: Option<&str>,
    offset: u64,
    limit: u64,
) -> Result<(Vec<RuntimeOperation>, u64), RuntimeOperationRepositoryError>;
```

Keep `get`, `create`, `delete`, `running`, and `has_running`. Remove `WorkspaceRepository::list`, `recent`, and `recent_for_workspace`. Create:

```rust
#[derive(Clone)]
pub struct RuntimeOperationQueryService {
    operations: Arc<dyn RuntimeOperationRepository>,
}

impl RuntimeOperationQueryService {
    pub fn new(operations: Arc<dyn RuntimeOperationRepository>) -> Self {
        Self { operations }
    }

    pub async fn page(
        &self,
        workspace_id: Option<&str>,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<RuntimeOperation>, u64), RuntimeOperationRepositoryError> {
        self.operations.page(workspace_id, offset, limit).await
    }

    pub async fn running(
        &self,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.operations.running().await
    }
}
```

- [ ] **Step 4: Implement service behavior**

`WorkspaceService::create(workflow)` validates the exact workflow and uses `Uuid::new_v4().to_string()`. Add:

```rust
pub async fn list_workflows(
    &self,
    offset: u64,
    limit: u64,
) -> Result<(Vec<WorkflowSummary>, u64), WorkspaceError> {
    let summaries = self.workflows.list_summaries().await
        .map_err(|_| WorkspaceError::CatalogUnavailable)?;
    let total = summaries.len() as u64;
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    Ok((summaries.into_iter().skip(offset).take(limit).collect(), total))
}

pub async fn list(
    &self,
    offset: u64,
    limit: u64,
) -> Result<(Vec<Workspace>, u64), WorkspaceError> {
    self.workspaces.page(offset, limit).await
        .map_err(|_| WorkspaceError::PersistenceUnavailable)
}
```

- [ ] **Step 5: Implement stable SQLite pages with dispatched batch hydration**

Use `PaginatorTrait`, `QueryOrder`, and `QuerySelect`. Count the filtered base query before offset/limit. Workspace order is `created_at DESC, id DESC`; operation order is `created_at DESC, id DESC`. Apply the optional operation workspace filter to count and data queries. Pass the selected anchors/operation IDs to grouped dispatcher load functions so each present runtime kind performs one provider query.

- [ ] **Step 6: Run tests and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml create_generates_a_uuid
cargo test --manifest-path src-tauri/Cargo.toml workflow_page_returns_total_before_paging
cargo test --manifest-path src-tauri/Cargo.toml workspace_page_is_stable_and_reports_total
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git add src-tauri/src/application src-tauri/src/adapters/sqlite src-tauri/tests/sqlite_runtime_dispatch.rs
git commit -m "feat(application): add paginated facade queries"
```

Expected: every command exits 0.

---

### Task 3: Add RunPod Placement To The Application Boundary

**Files:**
- Modify: `src-tauri/src/application/runtimes/runpod/model.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/ports/runtime_provider.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/errors.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`
- Modify: `src-tauri/src/adapters/runpod/runtime_provider.rs`

**Interfaces:**
- Consumes: stored `SecretKind::RunpodApiKey` and raw provider `PlacementResponse`.
- Produces: `RunpodRuntimeService::placement() -> Result<RunpodPlacement, RunpodRuntimeError>`.

- [ ] **Step 1: Write failing placement tests**

```rust
#[tokio::test]
async fn placement_reads_the_stored_key_and_returns_normalized_options() {
    let fakes = ProvisionFakes::ready();
    fakes.provider.set_placement(RunpodPlacement {
        max_volume_size_gb: 4_000,
        datacenters: vec![RunpodPlacementDatacenter {
            id: "EU-RO-1".into(),
            name: "EU Romania".into(),
            gpus: vec![RunpodPlacementGpu {
                id: "NVIDIA RTX 4090".into(),
                name: "RTX 4090".into(),
                vram_gb: 24,
            }],
        }],
    });

    let placement = fakes.service().placement().await.unwrap();
    assert_eq!(placement.max_volume_size_gb, 4_000);
    assert_eq!(placement.datacenters[0].gpus[0].vram_gb, 24);
    assert_eq!(fakes.provider.calls(), vec!["placement"]);
}

#[tokio::test]
async fn placement_requires_the_runpod_key_before_calling_provider() {
    let fakes = ProvisionFakes::ready_without_runpod_credential();
    assert_eq!(fakes.service().placement().await, Err(RunpodRuntimeError::CredentialMissing));
    assert!(fakes.provider.calls().is_empty());
}
```

- [ ] **Step 2: Run the test and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml placement_reads_the_stored_key_and_returns_normalized_options
```

Expected: FAIL because placement application models and provider behavior do not exist.

- [ ] **Step 3: Add exact placement models and port method**

```rust
pub const RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;

pub struct RunpodPlacement {
    pub max_volume_size_gb: u64,
    pub datacenters: Vec<RunpodPlacementDatacenter>,
}
pub struct RunpodPlacementDatacenter {
    pub id: String,
    pub name: String,
    pub gpus: Vec<RunpodPlacementGpu>,
}
pub struct RunpodPlacementGpu {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
}

async fn placement(
    &self,
    api_key: &SecretString,
) -> Result<RunpodPlacement, RunpodRuntimeProviderError>;
```

Derive the same diagnostics and clone/equality traits as neighboring application models.

- [ ] **Step 4: Implement service and adapter mapping**

```rust
pub async fn placement(&self) -> Result<RunpodPlacement, RunpodRuntimeError> {
    let key = self.secrets.get(SecretKind::RunpodApiKey).await?
        .ok_or(RunpodRuntimeError::CredentialMissing)?;
    self.provider.placement(&key).await.map_err(Into::into)
}
```

The adapter calls `RunpodProvider::placement`, builds a GPU lookup from complete `id`, `display_name`, and non-negative `memory_gb` values, maps complete datacenters and referenced GPU availability entries, and returns 4,000 GB. Malformed fields return `Unavailable`; unauthorized returns `Unauthorized`. Add `RunpodRuntimeError::InvalidCredential` and map provider errors exactly:

```rust
impl From<RunpodRuntimeProviderError> for RunpodRuntimeError {
    fn from(error: RunpodRuntimeProviderError) -> Self {
        match error {
            RunpodRuntimeProviderError::Unauthorized => Self::InvalidCredential,
            RunpodRuntimeProviderError::Unavailable
            | RunpodRuntimeProviderError::ProvisionerFailed => Self::ProviderUnavailable,
        }
    }
}
```

- [ ] **Step 5: Run tests and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git add src-tauri/src/application/runtimes/runpod src-tauri/src/adapters/runpod/runtime_provider.rs
git commit -m "feat(runpod): expose placement options"
```

Expected: PASS.

---
### Task 4: Define Facade DTOs And Command-Specific Errors

**Files:**
- Create: `src-tauri/src/facade/mod.rs`
- Create: `src-tauri/src/facade/model.rs`
- Create: `src-tauri/src/facade/errors.rs`
- Modify: `src-tauri/src/lib.rs` to add `pub mod facade`

**Interfaces:**
- Consumes: application workspace, shared runtime state, provider runtime, operation, placement, workflow, and identity models.
- Produces: Specta request/response types, safe conversions, `CommandError<Code>`, pagination validation, and per-command error enums.

- [ ] **Step 1: Write failing DTO and pagination tests**

```rust
#[test]
fn workspace_dto_exposes_shared_state_but_omits_provider_resource_ids() {
    let dto = WorkspaceDto::try_from(workspace_with_runpod_resources()).unwrap();
    let json = serde_json::to_value(dto).unwrap();
    assert_eq!(json["runtime"]["state"], "ready");
    assert_eq!(json["runtime"]["provider"]["runtimeKind"], "runpod");
    assert!(json["runtime"]["provider"].get("resources").is_none());
    assert!(!json.to_string().contains("endpoint-1"));
}

#[test]
fn operation_dto_keeps_runtime_kind_and_valid_progress_pair() {
    let dto = RuntimeOperationDto::try_from(running_provision_operation()).unwrap();
    assert_eq!(dto.runtime_kind, RuntimeKindDto::Runpod);
    assert!(matches!(
        dto.progress,
        RuntimeProgressDto::RunpodProvision {
            step: RunpodProvisionStepDto::CreateNetworkVolume
        }
    ));
}

#[test]
fn pagination_rejects_zero_and_more_than_one_hundred() {
    assert_eq!(validate_page(PageRequest { offset: 0, limit: 0 }), Err(InvalidPagination));
    assert_eq!(validate_page(PageRequest { offset: 0, limit: 101 }), Err(InvalidPagination));
}
```

- [ ] **Step 2: Run facade tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade::
```

Expected: FAIL because facade modules and DTOs do not exist.

- [ ] **Step 3: Add shared request and response shapes**

All DTOs derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`, `specta::Type`, and `DiagnosticDebug` where selected for diagnostics. Struct fields serialize as camel case; enum values serialize as snake case.

```rust
pub struct PageRequest { pub offset: u64, pub limit: u64 }
pub struct WorkflowPageDto { pub workflows: Vec<WorkflowDto>, pub total: u64 }
pub struct WorkspacePageDto { pub workspaces: Vec<WorkspaceDto>, pub total: u64 }
pub struct RuntimeOperationPageRequest {
    pub workspace_id: Option<String>,
    pub offset: u64,
    pub limit: u64,
}
pub struct RuntimeOperationPageDto {
    pub operations: Vec<RuntimeOperationDto>,
    pub total: u64,
}
pub struct CatalogRefDto { pub id: String, pub revision: String }
pub struct WorkflowDto {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}
pub struct CreateWorkspaceRequest { pub workflow: CatalogRefDto }
pub struct WorkspaceIdRequest { pub workspace_id: String }
pub struct ProvisionWorkspaceRequest {
    pub workspace_id: String,
    pub runtime: ProvisionRuntimeInput,
}
pub struct WorkspaceOperationDto {
    pub workspace: WorkspaceDto,
    pub operation: RuntimeOperationDto,
}
pub struct SetupApiKeyRequest {
    #[diagnostic(redact)]
    pub api_key: String,
}
```

Use a shared state plus tagged provider projection:

```rust
pub struct RuntimeDto {
    pub state: RuntimeStateDto,
    pub provider: RuntimeProviderDto,
}

#[serde(tag = "runtimeKind", rename_all = "snake_case")]
pub enum RuntimeProviderDto {
    Runpod {
        datacenter_id: String,
        gpu_id: String,
        volume_size_gb: u64,
    },
}

#[serde(tag = "runtimeKind", rename_all = "snake_case")]
pub enum ProvisionRuntimeInput {
    Runpod { datacenter_id: String, gpu_id: String, volume_size_gb: u64 },
}

#[serde(tag = "progressKind", rename_all = "snake_case")]
pub enum RuntimeProgressDto {
    RunpodProvision { step: RunpodProvisionStepDto },
    RunpodCleanup { step: RunpodCleanupStepDto },
}
```

`WorkspaceDto` contains ID, workflow, RFC3339 `created_at`, and optional `RuntimeDto`. `RuntimeOperationDto` contains string IDs, `runtime_kind`, operation kind/state, trace ID, tagged progress, and RFC3339 timestamps. Placement contains max size, datacenters, and GPUs. Identity contains only optional key name, username, and email.

Use fallible timestamp formatting and exact pagination validation:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FacadeMappingError {
    #[error("timestamp cannot be represented as RFC3339")]
    InvalidTimestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPagination;

pub fn validate_page(request: PageRequest) -> Result<(u64, u64), InvalidPagination> {
    (1..=100)
        .contains(&request.limit)
        .then_some((request.offset, request.limit))
        .ok_or(InvalidPagination)
}
```

- [ ] **Step 4: Add the error envelope and command-specific enums**

```rust
pub struct CommandError<Code> {
    pub code: Code,
    pub trace_id: String,
}

pub type CommandResult<T, Code> = Result<T, CommandError<Code>>;

fn error<Code>(code: Code) -> CommandError<Code> {
    CommandError {
        code,
        trace_id: crate::diagnostics::current_trace_uuid()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "trace-unavailable".to_owned()),
    }
}
```

Define these exact enum variants plus `CommandError` for unexpected mapping failure:

- `GetWorkflowsErrorCode`: `InvalidPagination`, `CatalogUnavailable`, `CommandError`.
- `GetWorkspacesErrorCode`: `InvalidPagination`, `PersistenceUnavailable`, `CommandError`.
- `CreateWorkspaceErrorCode`: `WorkflowNotFound`, `WorkspaceAlreadyExists`, `CatalogUnavailable`, `PersistenceUnavailable`, `CommandError`.
- `DeleteWorkspaceErrorCode`: `WorkspaceNotFound`, `RuntimeAttached`, `OperationRunning`, `PersistenceUnavailable`, `CommandError`.
- `ProvisionWorkspaceErrorCode`: `WorkspaceNotFound`, `WorkflowNotFound`, `AlreadyProvisioned`, `RuntimeFailed`, `OperationInProgress`, `CredentialMissing`, `CatalogUnavailable`, `PersistenceUnavailable`, `InvalidTransition`, `CommandError`.
- `CleanupWorkspaceErrorCode`: `WorkspaceNotFound`, `NotProvisioned`, `OperationInProgress`, `CredentialMissing`, `PersistenceUnavailable`, `InvalidTransition`, `CommandError`.
- `GetRuntimeOperationsErrorCode`: `InvalidPagination`, `PersistenceUnavailable`, `CommandError`.
- `GetRunpodPlacementErrorCode`: `CredentialMissing`, `InvalidCredential`, `ProviderUnavailable`, `CommandError`.
- Setup key errors: `AlreadyConfigured`, `InvalidCredential`, `IdentityUnavailable`, `StorageUnavailable`, `CommandError`.
- Get identity errors: `NotConfigured`, `InvalidCredential`, `IdentityUnavailable`, `StorageUnavailable`, `CommandError`.
- Delete key errors: `NotConfigured`, `StorageUnavailable`, `CommandError`.

Add exhaustive mapping functions. DTO conversion failure maps to that command's `CommandError` variant.

- [ ] **Step 5: Run tests and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade::
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git add src-tauri/src/facade src-tauri/src/lib.rs
git commit -m "feat(tauri): define facade contracts"
```

Expected: PASS; only facade modules derive transport types.

---

### Task 5: Implement Facade State, Closed Dispatch, And Root Commands

**Files:**
- Create: `src-tauri/src/facade/state.rs`
- Create: `src-tauri/src/facade/commands.rs`
- Modify: `src-tauri/src/facade/mod.rs`
- Modify: `src-tauri/src/application/secrets/service.rs`
- Modify: `src-tauri/src/application/workspace/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`

**Interfaces:**
- Consumes: Arc-owned application services and facade DTO/error mappings.
- Produces: all approved command implementations and a concrete facade dispatcher containing routing only.

- [ ] **Step 1: Write failing dispatcher tests**

```rust
#[test]
fn provision_dispatch_maps_the_runpod_input() {
    let command = runpod_provision_command(
        "workspace-1",
        ProvisionRuntimeInput::Runpod {
            datacenter_id: "EU-RO-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb: 100,
        },
    );
    assert_eq!(command.workspace_id, "workspace-1");
    assert_eq!(command.datacenter_id, "EU-RO-1");
    assert_eq!(command.gpu_id, "gpu-1");
    assert_eq!(command.volume_size_gb, 100);
}

#[test]
fn cleanup_dispatch_selects_the_attached_provider() {
    let workspace = workspace_with_runpod_runtime();
    assert_eq!(attached_runtime_kind(&workspace), Ok(RuntimeKind::Runpod));
}

#[test]
fn cleanup_dispatch_rejects_an_unprovisioned_workspace() {
    let workspace = Workspace { runtime: None, ..workspace_with_runpod_runtime() };
    assert_eq!(attached_runtime_kind(&workspace), Err(RunpodRuntimeError::NotProvisioned));
}
```

- [ ] **Step 2: Run dispatcher tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade::state::tests
```

Expected: FAIL because state and facade dispatch do not exist.

- [ ] **Step 3: Make application services Arc-owned and cloneable**

Use these owned service fields and constructors taking the same `Arc` values:

```rust
#[derive(Clone)]
pub struct WorkspaceService {
    workspaces: Arc<dyn WorkspaceRepository>,
    operations: Arc<dyn RuntimeOperationRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    events: Arc<dyn ApplicationEventSink>,
}

#[derive(Clone)]
pub struct SecretsService {
    store: Arc<dyn SecretStore>,
    runpod_identity: Arc<dyn SecretIdentityProvider>,
    hugging_face_identity: Arc<dyn SecretIdentityProvider>,
}
```

Preserve all existing behavior; do not add wrapper traits.

- [ ] **Step 4: Implement facade state and routing-only dispatcher**

```rust
#[derive(Clone)]
pub struct RuntimeDispatcher {
    runpod: RunpodRuntimeService,
}

pub struct FacadeState {
    workspaces: WorkspaceService,
    secrets: SecretsService,
    operations: RuntimeOperationQueryService,
    runtimes: RuntimeDispatcher,
}
```

`RuntimeDispatcher::provision` exhaustively matches `ProvisionRuntimeInput` and calls the corresponding service. `cleanup` receives a workspace loaded once by `FacadeState`, exhaustively matches `workspace.runtime.provider`, and calls the provider service. `recover_interrupted` groups running operations by `runtime_kind` and exhaustively routes each group. The dispatcher contains no credential reads, provider validation, direct provider-client calls, persistence calls, lifecycle mutation, or detached work.

Use this recovery flow:

```rust
pub async fn recover_interrupted(
    &self,
    operations: Vec<RuntimeOperation>,
) -> Result<(), RunpodRuntimeError> {
    let mut runpod = Vec::new();
    for operation in operations {
        match operation.runtime_kind {
            RuntimeKind::Runpod => runpod.push(operation),
        }
    }
    self.runpod.recover_interrupted(runpod).await
}
```

`FacadeState::recover_interrupted` calls `self.operations.running().await`, maps persistence failure to bootstrap recovery failure, and passes the returned operations to the dispatcher.

Implement every approved state method. Validate pagination before service calls. Convert API-key strings immediately to `SecretString`; never show or log the raw string.

- [ ] **Step 5: Add thin root commands**

Use this exact pattern for every command:

```rust
#[tauri::command]
#[specta::specta]
#[crate::diagnostics::diagnostic(root, show_output, show_error)]
pub async fn get_workspaces(
    state: tauri::State<'_, FacadeState>,
    #[diagnostic(show)] request: PageRequest,
) -> CommandResult<WorkspacePageDto, GetWorkspacesErrorCode> {
    state.get_workspaces(request).await
}
```

Setup-key commands show their request only through its redacting `DiagnosticDebug`. Commands contain no business branches, repository/provider calls, or manual span code.

- [ ] **Step 6: Run tests and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade::state::tests
cargo test --manifest-path src-tauri/Cargo.toml application::
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git add src-tauri/src/facade src-tauri/src/application
git commit -m "feat(tauri): add facade commands"
```

Expected: PASS.

---

### Task 6: Add Typed Events And The Tauri Specta Builder

**Files:**
- Create: `src-tauri/src/facade/events.rs`
- Modify: `src-tauri/src/facade/model.rs`
- Modify: `src-tauri/src/facade/mod.rs`
- Modify: `src-tauri/src/lib.rs` test module
- Generated: `src/generated/commands.ts`

**Interfaces:**
- Consumes: complete `ApplicationEvent` snapshots and facade DTO conversions.
- Produces: exact event names, synchronous event mapping, and a reusable Tauri Specta builder.

- [ ] **Step 1: Write failing event-name and export tests**

```rust
#[test]
fn facade_event_names_are_stable() {
    assert_eq!(WorkspaceChangedEvent::NAME, "workspace_changed");
    assert_eq!(WorkspaceDeletedEvent::NAME, "workspace_deleted");
    assert_eq!(RuntimeOperationEvent::NAME, "runtime_operation");
}

#[test]
fn export_bindings() {
    facade::export_typescript_bindings(&facade::builder())
        .expect("failed to export TypeScript bindings");
}
```

- [ ] **Step 2: Run tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade_event_names_are_stable
cargo test --manifest-path src-tauri/Cargo.toml export_bindings
```

Expected: FAIL because event DTOs and the builder are absent.

- [ ] **Step 3: Add exact event DTOs and sink**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[tauri_specta(event_name = "workspace_changed")]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChangedEvent { pub workspace: WorkspaceDto }

#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[tauri_specta(event_name = "workspace_deleted")]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedEvent { pub workspace_id: String }

#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[tauri_specta(event_name = "runtime_operation")]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationEvent { pub operation: RuntimeOperationDto }
```

`TauriEventSink` owns `tauri::AppHandle`. Match each application event, convert it with `TryFrom`, and call the typed event's `emit`. Mapping or emission failure logs only the error category and returns; it never logs a payload, retries, or changes persisted state.

- [ ] **Step 4: Build the registry**

```rust
pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::get_workflows,
            commands::get_workspaces,
            commands::create_workspace,
            commands::delete_workspace,
            commands::provision_workspace,
            commands::cleanup_workspace,
            commands::get_runtime_operations,
            commands::get_runpod_placement,
            commands::setup_runpod_api_key,
            commands::setup_hugging_face_api_key,
            commands::get_runpod_identity,
            commands::get_hugging_face_identity,
            commands::delete_runpod_api_key,
            commands::delete_hugging_face_api_key,
        ])
        .events(tauri_specta::collect_events![
            WorkspaceChangedEvent,
            WorkspaceDeletedEvent,
            RuntimeOperationEvent,
        ])
}
```

Export with `specta_typescript::Typescript::default()` to `../src/generated/commands.ts`.

- [ ] **Step 5: Run tests and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml facade_event_names_are_stable
cargo test --manifest-path src-tauri/Cargo.toml export_bindings
cargo test --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/facade src-tauri/src/lib.rs src/generated/commands.ts
git commit -m "feat(tauri): add generated facade events"
```

Expected: PASS.

---

### Task 7: Wire Fatal Native Bootstrap And Support Paths

**Files:**
- Modify: `src-tauri/src/diagnostics/mod.rs`
- Modify: `src-tauri/src/diagnostics/tests.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/AGENTS.md`
- Modify: `README.md`
- Modify: `src-tauri/README.md`

**Interfaces:**
- Consumes: concrete adapters, both closed dispatchers, services, facade builder/state, and Tauri event sink.
- Produces: `pub fn run()` with all-or-nothing setup and exact support paths.

- [ ] **Step 1: Write failing support-path and startup-order tests**

```rust
#[test]
fn support_file_names_are_stable() {
    assert_eq!(DB_FILE_NAME, "db.sqlite");
    assert_eq!(DIAGNOSTICS_FILE_NAME, "diagnostics.log");
}

#[test]
fn events_mount_before_interrupted_recovery() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    ).unwrap();
    assert!(
        source.find("mount_events(app)").unwrap()
            < source.find("recover_interrupted").unwrap()
    );
}
```

Update diagnostics tests to call `diagnostics::init(&temp_dir.join("diagnostics.log"))` and assert that exact file exists.

- [ ] **Step 2: Run tests and verify RED**

```sh
cargo test --manifest-path src-tauri/Cargo.toml support_file_names_are_stable
cargo test --manifest-path src-tauri/Cargo.toml events_mount_before_interrupted_recovery
```

Expected: FAIL because constants and active bootstrap do not exist.

- [ ] **Step 3: Initialize an exact diagnostics path**

```rust
pub fn init(log_path: &Path) -> Result<(), DiagnosticsInitializationError> {
    let directory = log_path.parent().ok_or_else(|| DiagnosticsInitializationError::SetupFailed {
        message: "diagnostics path has no parent".to_owned(),
    })?;
    let file_name = log_path.file_name().and_then(|value| value.to_str())
        .ok_or_else(|| DiagnosticsInitializationError::SetupFailed {
            message: "diagnostics file name is invalid".to_owned(),
        })?;
    let appender = logforth::append::file::FileBuilder::new(directory, file_name)
        .layout(logforth::layout::JsonLayout::default())
        .build()
        .map_err(|error| DiagnosticsInitializationError::SetupFailed {
            message: error.to_string(),
        })?;
    let logger = logforth::bridge::log::LogBridge::new(
        logforth::core::builder()
            .dispatch(|dispatch| {
                dispatch
                    .filter(ApplicationFilter)
                    .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
                    .append(appender)
            })
            .build(),
    );
    log::set_boxed_logger(Box::new(logger)).map_err(|error| {
        DiagnosticsInitializationError::InstallFailed {
            message: error.to_string(),
        }
    })?;
    log::set_max_level(log::LevelFilter::Info);
    Ok(())
}
```

- [ ] **Step 4: Implement the composition root**

```rust
const DB_FILE_NAME: &str = "db.sqlite";
const DIAGNOSTICS_FILE_NAME: &str = "diagnostics.log";
const BUNDLED_DIR_NAME: &str = "bundled";
```

`run()` creates `facade::builder()`, adds opener and debug-only MCP bridge, installs `builder.invoke_handler()`, and performs this exact setup order:

1. `builder.mount_events(app)`.
2. Resolve and create `app.path().app_data_dir()`.
3. Initialize `diagnostics.log`.
4. Connect `db.sqlite` with `SqliteInfraDatabase`.
5. Resolve `app.path().resource_dir()?.join("bundled")`.
6. Construct bundled, keyring, identity, provider, and SQLite adapters. SQLite construction includes RunPod-specific persistence behind the closed persistence dispatcher and generic workspace/transition/operation repositories.
7. Construct Arc-owned services, facade `RuntimeDispatcher`, and `FacadeState`.
8. Call `FacadeState::recover_interrupted().await` through `tauri::async_runtime::block_on`.
9. `app.manage(facade_state)` only after successful recovery.

Use this typed private error:

```rust
#[derive(Debug, thiserror::Error)]
enum BootstrapError {
    #[error("app data directory is unavailable")]
    AppDataDirectoryUnavailable,
    #[error("app data directory could not be created")]
    AppDataDirectoryCreateFailed,
    #[error("diagnostics initialization failed")]
    DiagnosticsInitializationFailed,
    #[error("database initialization failed")]
    DatabaseInitializationFailed,
    #[error("bundled resource directory is unavailable")]
    ResourceDirectoryUnavailable,
    #[error("provider initialization failed")]
    ProviderInitializationFailed,
    #[error("interrupted runtime recovery failed")]
    RuntimeRecoveryFailed,
}
```

Map sources at the composition root and log their diagnostic chain only after diagnostics is installed. Do not include source messages in facade contracts. Change `main` to:

```rust
fn main() {
    luma_forge_lib::run();
}
```

- [ ] **Step 5: Package resources and update support documentation**

Add under `bundle` in `tauri.conf.json`:

```json
"resources": {
  "../bundled/": "bundled/"
}
```

Replace every documented `logs/luma-forge.log` or dated `luma-forge.log` path in `src-tauri/AGENTS.md`, root `README.md`, and `src-tauri/README.md` with the single file `<app_data_dir>/diagnostics.log`. Document `db.sqlite` beside it. Do not change unrelated guidance.

- [ ] **Step 6: Run native verification and commit**

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
! rg -n "luma-forge\.log|app_data_dir.*/logs" README.md src-tauri/README.md src-tauri/AGENTS.md
git add src-tauri/src/diagnostics src-tauri/src/lib.rs src-tauri/src/main.rs src-tauri/tauri.conf.json src-tauri/AGENTS.md README.md src-tauri/README.md
git commit -m "feat(tauri): wire native facade startup"
```

Expected: tests, formatting, and Clippy exit 0; `rg` prints no matches.

---

### Task 8: Regenerate TypeScript And Update The Existing Probe

**Files:**
- Generated: `src/generated/commands.ts`
- Modify: `src/pages/home/ui/home-page.tsx`

**Interfaces:**
- Consumes: final Tauri Specta builder.
- Produces: checked-in bindings and a compiling developer probe; no product UI feature.

- [ ] **Step 1: Regenerate bindings and observe the expected frontend failure**

```sh
bun run codegen:commands
bun run build
```

Expected: codegen PASS; build FAIL because the probe still imports and calls legacy contracts.

- [ ] **Step 2: Update only the command probe contract**

Use these exact calls:

```ts
const pageRequest = { offset: 0, limit: 20 };

commands.getWorkflows(pageRequest);
commands.getWorkspaces(pageRequest);
commands.getRuntimeOperations({ workspaceId: null, offset: 0, limit: 20 });
commands.createWorkspace({ workflow: { id: "", revision: "" } });
commands.provisionWorkspace({
  workspaceId: "",
  runtime: {
    runtimeKind: "runpod",
    datacenterId: "",
    gpuId: "",
    volumeSizeGb: 1,
  },
});
commands.cleanupWorkspace({ workspaceId: "" });
commands.deleteWorkspace({ workspaceId: "" });
```

Rename identity, placement, and API-key probes to the approved command names. Subscribe to `events.workspaceChangedEvent`, `events.workspaceDeletedEvent`, and `events.runtimeOperationEvent` exactly as generated. Add no stores, forms, product UI, or response/event reconciliation.

- [ ] **Step 3: Run full verification**

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
bun run codegen:commands
bun run build
bun run lint
```

Expected: every command exits 0; the second codegen leaves generated bindings unchanged.

- [ ] **Step 4: Confirm generated safety, provider boundaries, and scope**

```sh
test "$(rg -c "apiKey: string" src/generated/commands.ts)" -eq 1
! rg -n "networkVolumeId|provisionerPodId|templateId|endpointId" src/generated/commands.ts
! rg -n "runpod_workspace|RunpodRuntime|RunpodProgress|network_volume_id|provisioner_pod_id|template_id|endpoint_id" \
  src-tauri/src/adapters/sqlite/workspace_repository.rs \
  src-tauri/src/adapters/sqlite/runtime_transition_repository.rs \
  src-tauri/src/adapters/sqlite/runtime_operation_repository.rs
git diff --check
git status --short
```

Expected: `apiKey` appears only in setup request input; provider resource IDs do not appear in output types; all three generic SQLite files contain no RunPod implementation matches; diff check is silent; status contains only generated/probe changes.

- [ ] **Step 5: Commit bindings and probe adaptation**

```sh
git add src/generated/commands.ts src/pages/home/ui/home-page.tsx
git commit -m "chore(tauri): regenerate facade bindings"
```

---

## Final Review Checklist

- `Workspace` contains `Option<Runtime>` with shared `RuntimeState` and tagged `RuntimeProvider`.
- `workspace_runtimes` contains only `workspace_id`, `runtime_kind`, and shared `state`.
- `runtime_operations` persists `runtime_kind`; provider progress remains readable after cleanup.
- Provider extension tables contain no shared lifecycle state and cascade from the generic anchor.
- Generic SQLite transition orchestration owns one transaction and delegates all provider persistence.
- Generic workspace, transition, and operation repository files contain no RunPod SQL, mapping, lifecycle branches, resource fields, or progress-step strings.
- Workspace pages are hydrated through grouped provider dispatch without N+1 reads.
- Workflow, workspace, and operation responses carry total counts and approved pagination.
- Runtime operation progress is provider-resolved and type-safe.
- Commands expose only facade DTOs and command-specific error enums.
- API keys are write-only and redacted at command roots.
- Provider resource IDs remain native-only.
- Events are mounted before recovery and emitted workspace-first after commits.
- Support files are exactly `db.sqlite` and `diagnostics.log` under `app_data_dir`, and documentation matches.
- Packaged app resolves `resource_dir/bundled`.
- Native tests, formatting, Clippy, codegen, frontend build, lint, provider-boundary audit, and diff check all pass.

---
