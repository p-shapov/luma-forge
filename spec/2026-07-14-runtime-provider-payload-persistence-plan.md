# Runtime Provider Payload Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist typed runtime-provider state and operation progress as opaque tagged JSON on the existing provider-neutral SQLite rows, removing all provider-specific persistence tables and dispatch.

**Architecture:** Pin the JSON representation on the closed application enums and expose only provider-neutral discriminator and validation methods there. SQLite serializes validated typed values before opening the existing transition transaction, stores them inline on `workspace_runtimes` and `runtime_operations`, and deserializes plus cross-checks them during the existing workspace and operation queries.

**Tech Stack:** Rust 2021, Serde/serde_json, SeaORM 2.0 RC, SQLite, Tokio integration tests, existing diagnostic macros, Cargo.

## Global Constraints

- `workspace_runtimes` remains the one-runtime-per-workspace anchor and admission lock.
- `runtime_operations` remains durable operation history after successful cleanup and workspace/runtime deletion.
- Runtime transitions remain one transaction; validation and serialization happen before `BEGIN`, and events remain best-effort only after commit.
- Neutral columns remain authoritative for SQL queries, admission, ordering, filtering, totals, and recovery selection.
- `provider_payload` and `progress_payload` are non-null `TEXT` columns containing the tagged JSON representation of `RuntimeProvider` and `RuntimeProgress` respectively.
- JSON names are explicitly pinned in `snake_case`; invalid JSON, unknown fields or variants, invalid field types, unsupported shapes, and neutral/payload discriminator mismatches fail closed as the existing `CorruptData` category.
- No payload version field is added before a real incompatible-payload preservation requirement exists.
- No raw secret, API key, bearer token, or credential may enter diagnostics, errors, public DTOs, command responses, generated frontend types, or test fixtures; no raw JSON payload may enter diagnostics, errors, public DTOs, command responses, or generated frontend types.
- Application models gain only Serde; they gain no SeaORM, SQLite, Tauri, Specta, keyring, or provider-client dependency.
- Repository port signatures, lifecycle dispatch, remote side-effect ordering, retries, idempotency, recovery behavior, Tauri commands, DTOs, events, generated bindings, and frontend behavior remain unchanged.
- Existing local databases are deleted and recreated; add no migration, data copy, dual-read, legacy fallback, compatibility shim, or silent fallback.
- Add no provider-specific SQLite table, entity, relation, SQL, adapter module, dispatch arm, or hydration query.
- Add no query-count test, SQL-text test, exact JSON-string assertion, or assertion whose purpose is proving removed tables are absent.
- Use only the already-installed `serde` and `serde_json` dependencies.
- Use Conventional Commits for every implementation commit.
- Final native verification is exactly:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

---

## File Structure

- Modify `src-tauri/src/application/runtimes/model.rs`: pin the top-level provider/progress tags, own the stable `RuntimeKind` database identifier, and expose generic transition/progress invariant checks.
- Modify `src-tauri/src/application/runtimes/runpod/model.rs`: pin the RunPod payload fields, operation-family tags, and lifecycle-step names without adding persistence dependencies.
- Modify `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs`: add the non-null `provider_payload` column and remove the RunPod extension relation.
- Modify `src-tauri/src/infra/sqlite/entities/runtime_operations.rs`: add the non-null `progress_payload` column.
- Modify `src-tauri/src/infra/sqlite/entities/mod.rs`: register only provider-neutral runtime entities.
- Delete `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`: provider runtime state no longer has its own table.
- Delete `src-tauri/src/infra/sqlite/entities/runpod_runtime_operation_progress.rs`: provider operation progress no longer has its own table.
- Modify `src-tauri/src/adapters/sqlite/runtime_transition_repository.rs`: validate and serialize both payloads before the transaction, then write them with the neutral rows inside the existing admission/transition transaction.
- Modify `src-tauri/src/adapters/sqlite/workspace_repository.rs`: hydrate `RuntimeProvider` directly from the joined anchor row and cross-check its provider against `runtime_kind`.
- Modify `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs`: hydrate `RuntimeProgress` directly from each selected operation row and cross-check both discriminators.
- Modify `src-tauri/src/adapters/sqlite/mod.rs`: remove provider-persistence and persistence-dispatch modules.
- Delete `src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs`: generic serde_json replaces RunPod SQL mapping.
- Delete `src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs`: SQLite no longer dispatches provider persistence.
- Rename `src-tauri/tests/sqlite_runtime_dispatch.rs` to `src-tauri/tests/sqlite_runtime_payload_persistence.rs`: keep the current admission, rollback, cleanup, pagination, and deletion coverage while replacing extension-table assertions with inline-payload behavior.
- Do not modify `src-tauri/src/infra/sqlite/database.rs`, repository ports, facade code, diagnostics code, `src/generated/commands.ts`, or frontend code.

### Task 1: Pin the typed payload contract and generic invariants

**Files:**

- Modify: `src-tauri/src/application/runtimes/model.rs:1-333`
- Modify: `src-tauri/src/application/runtimes/runpod/model.rs:1-126`

**Interfaces:**

- Consumes: existing closed `RuntimeProvider`, `RuntimeProgress`, `RuntimeKind`, `RuntimeOperation`, `RunpodRuntime`, and `RunpodProgress` application types.
- Produces: `RuntimeKind::as_str() -> &'static str`, `FromStr<Err = ()> for RuntimeKind`, `RuntimeProgress::runtime_kind() -> RuntimeKind`, `RuntimeProgress::operation_kind() -> RuntimeOperationKind`, `RuntimeOperation::validate_progress() -> Result<(), RuntimeOperationError>`, and `RuntimeOperation::validate_transition(&Workspace) -> Result<(), RuntimeOperationError>` for Task 2.

- [ ] **Step 1: Add failing serialization and invariant tests**

In the existing `tests` module in `src-tauri/src/application/runtimes/model.rs`, replace its RunPod import block and add the `Workspace` import:

```rust
use crate::application::{
    runtimes::runpod::{
        RunpodCleanupStep, RunpodContractRequirements, RunpodProgress, RunpodProvisionStep,
        RunpodRuntime, RunpodRuntimeConfig,
    },
    workspace::Workspace,
};
```

Add this fixture and these tests to the same module:

```rust
fn provider_payload_fixture() -> RuntimeProvider {
    let mut runtime = RunpodRuntime::new_provisioning(RunpodRuntimeConfig {
        datacenter_id: "EU-RO-1".into(),
        gpu_id: "gpu-1".into(),
        volume_size_gb: 100,
    });
    runtime.resources.network_volume_id = Some("network-volume-1".into());
    runtime.resources.template_id = Some("template-1".into());
    RuntimeProvider::Runpod(runtime)
}

#[test]
fn runtime_kind_uses_the_pinned_neutral_identifier() {
    assert_eq!(RuntimeKind::Runpod.as_str(), "runpod");
    assert_eq!("runpod".parse::<RuntimeKind>(), Ok(RuntimeKind::Runpod));
    assert_eq!("Runpod".parse::<RuntimeKind>(), Err(()));
}

#[test]
fn provider_payload_is_tagged_round_trippable_and_strict() {
    let provider = provider_payload_fixture();
    let value = serde_json::to_value(&provider).unwrap();

    assert_eq!(value["provider"], "runpod");
    assert_eq!(value["payload"]["config"]["datacenter_id"], "EU-RO-1");
    assert_eq!(value["payload"]["resources"]["template_id"], "template-1");
    assert_eq!(
        serde_json::from_value::<RuntimeProvider>(value.clone()).unwrap(),
        provider
    );

    let mut unknown_field = value.clone();
    unknown_field["payload"]["config"]["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RuntimeProvider>(unknown_field).is_err());

    let mut invalid_type = value;
    invalid_type["payload"]["config"]["volume_size_gb"] = serde_json::json!("100");
    assert!(serde_json::from_value::<RuntimeProvider>(invalid_type).is_err());
    assert!(serde_json::from_value::<RuntimeProvider>(serde_json::json!({
        "provider": "unknown",
        "payload": {}
    }))
    .is_err());
    assert!(serde_json::from_value::<RuntimeProvider>(serde_json::json!({
        "provider": "runpod",
        "config": {}
    }))
    .is_err());
}

#[test]
fn progress_payload_is_tagged_round_trippable_and_strict() {
    let progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
        RunpodProvisionStep::CreateNetworkVolume,
    ));
    let value = serde_json::to_value(progress).unwrap();

    assert_eq!(value["provider"], "runpod");
    assert_eq!(value["payload"]["operation"], "provision");
    assert_eq!(value["payload"]["step"], "create_network_volume");
    assert_eq!(
        serde_json::from_value::<RuntimeProgress>(value.clone()).unwrap(),
        progress
    );

    let mut unknown_field = value;
    unknown_field["payload"]["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<RuntimeProgress>(unknown_field).is_err());
    assert!(serde_json::from_value::<RuntimeProgress>(serde_json::json!({
        "provider": "runpod",
        "payload": {
            "operation": "provision",
            "step": "unknown"
        }
    }))
    .is_err());
}

#[test]
fn runtime_operation_validates_provider_neutral_transition_invariants() {
    let mut workspace = Workspace {
        id: "workspace-1".into(),
        workflow: CatalogRef::new("workflow-1", "1"),
        created_at: OffsetDateTime::UNIX_EPOCH,
        runtime: Some(Runtime {
            state: RuntimeState::CleaningUp,
            provider: provider_payload_fixture(),
        }),
    };
    let mut operation = RuntimeOperation::running(
        Uuid::from_u128(1),
        "workspace-1",
        RuntimeKind::Runpod,
        RuntimeOperationKind::Cleanup,
        RuntimeProgress::Runpod(RunpodProgress::Cleanup(
            RunpodCleanupStep::DeleteEndpoint,
        )),
        OffsetDateTime::UNIX_EPOCH,
    );

    assert_eq!(operation.validate_progress(), Ok(()));
    assert_eq!(operation.validate_transition(&workspace), Ok(()));

    operation.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
        RunpodProvisionStep::CreateNetworkVolume,
    ));
    assert_eq!(
        operation.validate_progress(),
        Err(RuntimeOperationError::InvalidTransition)
    );
    operation.progress = RuntimeProgress::Runpod(RunpodProgress::Cleanup(
        RunpodCleanupStep::DeleteEndpoint,
    ));

    workspace.id = "workspace-2".into();
    assert_eq!(
        operation.validate_transition(&workspace),
        Err(RuntimeOperationError::InvalidTransition)
    );
    workspace.id = "workspace-1".into();
    workspace.runtime = None;
    assert_eq!(
        operation.validate_transition(&workspace),
        Err(RuntimeOperationError::InvalidTransition)
    );

    operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();
    assert_eq!(operation.validate_transition(&workspace), Ok(()));
}
```

- [ ] **Step 2: Run the focused tests and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib application::runtimes::model::tests
```

Expected: compilation fails because the payload types do not implement Serde, `RuntimeKind::as_str`/`FromStr` do not exist, and the invariant methods do not exist.

- [ ] **Step 3: Pin the RunPod payload representation**

At the top of `src-tauri/src/application/runtimes/runpod/model.rs`, replace the existing import with:

```rust
use serde::{Deserialize, Serialize};

use crate::application::runtimes::{CatalogRef, RuntimeOperationKind};
```

Replace the RunPod progress and persisted runtime definitions with the following definitions; leave placement, catalog-requirement, and runtime-definition types unchanged:

```rust
#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum RunpodProvisionStep {
    #[serde(rename = "create_network_volume")]
    CreateNetworkVolume,
    #[serde(rename = "start_provisioner_pod")]
    StartProvisionerPod,
    #[serde(rename = "poll_provisioner")]
    PollProvisioner,
    #[serde(rename = "terminate_provisioner_pod")]
    TerminateProvisionerPod,
    #[serde(rename = "create_template")]
    CreateTemplate,
    #[serde(rename = "create_endpoint")]
    CreateEndpoint,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum RunpodCleanupStep {
    #[serde(rename = "delete_endpoint")]
    DeleteEndpoint,
    #[serde(rename = "delete_template")]
    DeleteTemplate,
    #[serde(rename = "terminate_provisioner_pod")]
    TerminateProvisionerPod,
    #[serde(rename = "delete_network_volume")]
    DeleteNetworkVolume,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "operation", content = "step", deny_unknown_fields)]
pub enum RunpodProgress {
    #[serde(rename = "provision")]
    Provision(#[diagnostic(show)] RunpodProvisionStep),
    #[serde(rename = "cleanup")]
    Cleanup(#[diagnostic(show)] RunpodCleanupStep),
}

impl RunpodProgress {
    pub fn operation_kind(self) -> RuntimeOperationKind {
        match self {
            Self::Provision(_) => RuntimeOperationKind::Provision,
            Self::Cleanup(_) => RuntimeOperationKind::Cleanup,
        }
    }

    pub fn provision_step(self) -> Option<RunpodProvisionStep> {
        match self {
            Self::Provision(step) => Some(step),
            Self::Cleanup(_) => None,
        }
    }

    pub fn cleanup_step(self) -> Option<RunpodCleanupStep> {
        match self {
            Self::Provision(_) => None,
            Self::Cleanup(step) => Some(step),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntimeConfig {
    #[diagnostic(show)]
    #[serde(rename = "datacenter_id")]
    pub datacenter_id: String,
    #[diagnostic(show)]
    #[serde(rename = "gpu_id")]
    pub gpu_id: String,
    #[diagnostic(show)]
    #[serde(rename = "volume_size_gb")]
    pub volume_size_gb: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntimeResources {
    #[diagnostic(show)]
    #[serde(rename = "network_volume_id")]
    pub network_volume_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "provisioner_pod_id")]
    pub provisioner_pod_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "template_id")]
    pub template_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "endpoint_id")]
    pub endpoint_id: Option<String>,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntime {
    #[diagnostic(show)]
    #[serde(rename = "config")]
    pub config: RunpodRuntimeConfig,
    #[diagnostic(show)]
    #[serde(rename = "resources")]
    pub resources: RunpodRuntimeResources,
}
```

Keep the existing `RunpodRuntime::new_provisioning` implementation unchanged.

- [ ] **Step 4: Pin the closed enum tags and add generic validation**

At the top of `src-tauri/src/application/runtimes/model.rs`, add Serde and `Workspace` imports:

```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::workspace::Workspace;
```

Replace `RuntimeKind` and add its stable neutral conversion:

```rust
#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Runpod,
}

impl RuntimeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

impl std::str::FromStr for RuntimeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "runpod" => Ok(Self::Runpod),
            _ => Err(()),
        }
    }
}
```

Replace `RuntimeProvider` with the explicitly tagged form while retaining its existing methods:

```rust
#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "provider", content = "payload", deny_unknown_fields)]
pub enum RuntimeProvider {
    #[serde(rename = "runpod")]
    Runpod(#[diagnostic(show)] RunpodRuntime),
}

impl RuntimeProvider {
    pub fn kind(&self) -> RuntimeKind {
        match self {
            Self::Runpod(_) => RuntimeKind::Runpod,
        }
    }

    pub fn as_runpod(&self) -> Option<&RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }

    pub fn as_runpod_mut(&mut self) -> Option<&mut RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}
```

Replace `RuntimeProgress` and add its provider-neutral accessors:

```rust
#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "provider", content = "payload", deny_unknown_fields)]
pub enum RuntimeProgress {
    #[serde(rename = "runpod")]
    Runpod(#[diagnostic(show)] RunpodProgress),
}

impl RuntimeProgress {
    pub fn runtime_kind(self) -> RuntimeKind {
        match self {
            Self::Runpod(_) => RuntimeKind::Runpod,
        }
    }

    pub fn operation_kind(self) -> RuntimeOperationKind {
        match self {
            Self::Runpod(progress) => progress.operation_kind(),
        }
    }
}
```

Add these methods to the existing `impl RuntimeOperation` before `running`:

```rust
pub fn validate_progress(&self) -> Result<(), RuntimeOperationError> {
    (self.runtime_kind == self.progress.runtime_kind()
        && self.kind == self.progress.operation_kind())
    .then_some(())
    .ok_or(RuntimeOperationError::InvalidTransition)
}

pub fn validate_transition(&self, workspace: &Workspace) -> Result<(), RuntimeOperationError> {
    self.validate_progress()?;
    (workspace.id == self.workspace_id
        && match &workspace.runtime {
            Some(runtime) => runtime.kind() == self.runtime_kind,
            None => {
                self.kind == RuntimeOperationKind::Cleanup
                    && self.state == RuntimeOperationState::Succeeded
            }
        })
    .then_some(())
    .ok_or(RuntimeOperationError::InvalidTransition)
}
```

- [ ] **Step 5: Run the model tests and formatter**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib application::runtimes::model::tests
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all runtime-model tests pass, including strict tagged payload round trips and transition validation; formatting reports no diff.

- [ ] **Step 6: Commit the typed payload contract**

Run:

```bash
git add src-tauri/src/application/runtimes/model.rs src-tauri/src/application/runtimes/runpod/model.rs
git commit -m "feat(runtime): define provider payload contract"
```

Expected: one Conventional Commit containing only the application-model contract and its unit tests.

### Task 2: Cut SQLite persistence over to inline payloads

**Files:**

- Modify: `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs:1-23`
- Modify: `src-tauri/src/infra/sqlite/entities/runtime_operations.rs:1-20`
- Modify: `src-tauri/src/infra/sqlite/entities/mod.rs:1-5`
- Delete: `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`
- Delete: `src-tauri/src/infra/sqlite/entities/runpod_runtime_operation_progress.rs`
- Modify: `src-tauri/src/adapters/sqlite/runtime_transition_repository.rs:1-292`
- Modify: `src-tauri/src/adapters/sqlite/workspace_repository.rs:1-237`
- Modify: `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs:1-204`
- Modify: `src-tauri/src/adapters/sqlite/mod.rs:1-9`
- Delete: `src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs`
- Delete: `src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs`
- Rename: `src-tauri/tests/sqlite_runtime_dispatch.rs` to `src-tauri/tests/sqlite_runtime_payload_persistence.rs`

**Interfaces:**

- Consumes: Task 1's `RuntimeKind::as_str`/`FromStr`, `RuntimeProvider` and `RuntimeProgress` Serde implementations, and `RuntimeOperation` validation methods; existing `WorkspaceRepository`, `RuntimeOperationRepository`, and `RuntimeTransitionRepository` ports remain unchanged.
- Produces: `workspace_runtimes.provider_payload: String` and `runtime_operations.progress_payload: String`; `SqliteWorkspaceRepository`, `SqliteRuntimeOperationRepository`, and `SqliteRuntimeTransitionRepository` keep their existing public constructors and trait signatures.

- [ ] **Step 1: Rename the integration test and replace extension-table setup**

Rename the test file:

```bash
git mv src-tauri/tests/sqlite_runtime_dispatch.rs src-tauri/tests/sqlite_runtime_payload_persistence.rs
```

Replace the import block in the renamed file with:

```rust
use luma_forge_lib::{
    adapters::sqlite::{
        SqliteRuntimeOperationRepository, SqliteRuntimeTransitionRepository,
        SqliteWorkspaceRepository,
    },
    application::{
        runtimes::{
            ports::{
                RuntimeOperationRepository, RuntimeOperationRepositoryError,
                RuntimePersistenceError, RuntimeTransitionRepository,
            },
            runpod::{
                RunpodCleanupStep, RunpodProgress, RunpodProvisionStep, RunpodRuntime,
                RunpodRuntimeConfig,
            },
            CatalogRef, Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationKind,
            RuntimeProgress, RuntimeProvider, RuntimeState,
        },
        workspace::{
            ports::{WorkspaceRepository, WorkspaceRepositoryError},
            Workspace,
        },
    },
    infra::sqlite::{
        database::SqliteInfraDatabase,
        entities::{runtime_operations, workspace_runtimes},
    },
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, EntityTrait, IntoActiveModel,
};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;
```

Delete `Fixture::with_orphaned_anchor`. Keep `Fixture::new`, `Fixture::with_ready_runtime`, `Fixture::workspace`, and `runpod_runtime`. Replace `running_operation` with:

```rust
fn running_operation(workspace_id: &str, kind: RuntimeOperationKind) -> RuntimeOperation {
    let progress = match kind {
        RuntimeOperationKind::Provision => {
            RunpodProgress::Provision(RunpodProvisionStep::CreateNetworkVolume)
        }
        RuntimeOperationKind::Cleanup => {
            RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint)
        }
    };
    RuntimeOperation::running(
        Uuid::new_v4(),
        workspace_id,
        RuntimeKind::Runpod,
        kind,
        RuntimeProgress::Runpod(progress),
        OffsetDateTime::UNIX_EPOCH,
    )
}
```

- [ ] **Step 2: Replace dispatch-specific tests with inline-payload behavior tests**

Replace `workspace_hydrates_state_and_runpod_extension_through_dispatch` with:

```rust
#[tokio::test]
async fn workspace_get_and_page_round_trip_inline_provider_payload() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let runpod = workspace
        .runtime
        .as_mut()
        .unwrap()
        .provider
        .as_runpod_mut()
        .unwrap();
    runpod.resources.network_volume_id = Some("network-volume-1".into());
    runpod.resources.template_id = Some("template-1".into());
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);

    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    assert_eq!(
        fixture.workspaces.get(&workspace.id).await.unwrap(),
        Some(workspace.clone())
    );
    assert_eq!(fixture.workspaces.page(0, 10).await.unwrap(), (vec![workspace.clone()], 1));

    let anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(anchor.runtime_kind, "runpod");
    assert_eq!(anchor.state, "provisioning");
    assert_eq!(
        serde_json::from_str::<RuntimeProvider>(&anchor.provider_payload).unwrap(),
        workspace.runtime.unwrap().provider
    );
}
```

Add this operation read-shape test after `workspace_page_is_stable_and_reports_total`:

```rust
#[tokio::test]
async fn operation_reads_keep_progress_filtering_ordering_totals_and_recovery() {
    let fixture = Fixture::with_ready_runtime().await;

    let mut cleanup_workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    cleanup_workspace.runtime.as_mut().unwrap().state = RuntimeState::CleaningUp;
    let mut cleanup = running_operation("workspace-1", RuntimeOperationKind::Cleanup);
    cleanup.created_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
    cleanup.updated_at = cleanup.created_at;
    fixture
        .transitions
        .save_transition(&cleanup_workspace, &cleanup)
        .await
        .unwrap();

    let mut provision_workspace = fixture.workspace("workspace-2");
    fixture
        .workspaces
        .create(provision_workspace.clone())
        .await
        .unwrap();
    provision_workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 120));
    let mut provision = running_operation("workspace-2", RuntimeOperationKind::Provision);
    provision.created_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
    provision.updated_at = provision.created_at;
    fixture
        .transitions
        .save_transition(&provision_workspace, &provision)
        .await
        .unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 2).await.unwrap(),
        (vec![provision.clone(), cleanup.clone()], 3)
    );
    assert_eq!(
        fixture.operations.page(None, 1, 1).await.unwrap(),
        (vec![cleanup.clone()], 3)
    );
    let (workspace_operations, workspace_total) = fixture
        .operations
        .page(Some("workspace-1"), 0, 10)
        .await
        .unwrap();
    assert_eq!(workspace_total, 2);
    assert_eq!(workspace_operations.first(), Some(&cleanup));

    let mut running = fixture.operations.running().await.unwrap();
    running.sort_by_key(|operation| operation.created_at);
    assert_eq!(running, vec![cleanup, provision]);
    assert_eq!(fixture.operations.has_running("workspace-1").await, Ok(true));
    assert_eq!(fixture.operations.has_running("workspace-2").await, Ok(true));
}
```

Replace `provider_failure_rolls_back_anchor_and_operation` with a real database-failure test:

```rust
#[tokio::test]
async fn database_failure_rolls_back_anchor_and_operation() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);
    fixture
        .database
        .connection()
        .execute_unprepared(
            "CREATE TRIGGER fail_workspace_runtime_insert
             BEFORE INSERT ON workspace_runtimes
             BEGIN
                 SELECT RAISE(ABORT, 'forced persistence failure');
             END",
        )
        .await
        .unwrap();

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await,
        Err(RuntimePersistenceError::Unavailable),
    );
    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    assert!(runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
}
```

Replace `operation_kind_and_progress_family_mismatch_rolls_back` with:

```rust
#[tokio::test]
async fn progress_family_mismatch_is_rejected_on_write_and_read() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::CleaningUp, 100));
    let mut invalid = running_operation(&workspace.id, RuntimeOperationKind::Cleanup);
    invalid.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
        RunpodProvisionStep::CreateNetworkVolume,
    ));

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &invalid)
            .await,
        Err(RuntimePersistenceError::CorruptData),
    );
    assert!(runtime_operations::Entity::find_by_id(invalid.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());

    workspace.runtime.as_mut().unwrap().state = RuntimeState::Provisioning;
    let valid = running_operation(&workspace.id, RuntimeOperationKind::Provision);
    fixture
        .transitions
        .save_transition(&workspace, &valid)
        .await
        .unwrap();
    let mut stored = runtime_operations::Entity::find_by_id(valid.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    stored.operation_kind = Set("cleanup".into());
    stored.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 10).await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.operations.running().await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
}
```

Add this rejected provision test before the cleanup-admission test:

```rust
#[tokio::test]
async fn provision_admission_rejection_rolls_back_operation_and_anchor_changes() {
    let fixture = Fixture::with_ready_runtime().await;
    let original = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    let mut attempted = original.clone();
    attempted
        .runtime
        .as_mut()
        .unwrap()
        .provider
        .as_runpod_mut()
        .unwrap()
        .config
        .volume_size_gb = 200;
    let operation = running_operation("workspace-1", RuntimeOperationKind::Provision);

    assert_eq!(
        fixture
            .transitions
            .save_transition(&attempted, &operation)
            .await,
        Err(RuntimePersistenceError::OperationAlreadyRunning)
    );
    assert_eq!(fixture.workspaces.get("workspace-1").await, Ok(Some(original)));
    assert!(runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
}
```

In `cleanup_admission_rejects_an_anchor_already_in_transition`, replace the manual `RuntimeOperation::running` construction with:

```rust
let operation = running_operation("workspace-1", RuntimeOperationKind::Cleanup);
```

After its existing rejected-operation assertion, add:

```rust
assert_eq!(fixture.workspaces.get("workspace-1").await, Ok(Some(workspace)));
```

Replace `cleanup_removes_runtime_but_keeps_dispatched_operation_progress` with:

```rust
#[tokio::test]
async fn successful_cleanup_removes_anchor_and_keeps_terminal_progress() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    workspace.runtime = None;
    let mut operation = running_operation("workspace-1", RuntimeOperationKind::Cleanup);
    operation.progress = RuntimeProgress::Runpod(RunpodProgress::Cleanup(
        RunpodCleanupStep::DeleteNetworkVolume,
    ));
    operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.workspaces.get("workspace-1").await, Ok(Some(workspace)));
    let stored = fixture
        .operations
        .page(Some("workspace-1"), 0, 10)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|stored| stored.id == operation.id)
        .unwrap();
    assert_eq!(stored, operation);
}
```

Replace `anchor_without_provider_extension_is_corrupt` with these three corruption tests:

```rust
#[tokio::test]
async fn malformed_provider_payload_fails_workspace_get_and_page() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    anchor.provider_payload = Set("{".into());
    anchor.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.workspaces.page(0, 10).await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn provider_payload_tag_disagreement_is_corrupt() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    anchor.runtime_kind = Set("other".into());
    anchor.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn malformed_progress_payload_fails_operation_page_and_running() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);
    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    let mut stored = runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    stored.progress_payload = Set("{".into());
    stored.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 10).await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.operations.running().await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
}
```

Keep `workspace_page_is_stable_and_reports_total` and `workspace_delete_distinguishes_eligible_and_missing_rows` unchanged.

- [ ] **Step 3: Run the renamed integration test and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test sqlite_runtime_payload_persistence
```

Expected: compilation fails because `workspace_runtimes::Model` has no `provider_payload` and `runtime_operations::Model` has no `progress_payload`.

- [ ] **Step 4: Replace provider extension entities with inline columns**

Replace `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs` with:

```rust
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspace_runtimes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    pub runtime_kind: String,
    pub state: String,
    pub provider_payload: String,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id", on_delete = "Cascade")]
    pub workspace: HasOne<super::workspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
```

Replace `src-tauri/src/infra/sqlite/entities/runtime_operations.rs` with:

```rust
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "runtime_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workspace_id: String,
    pub runtime_kind: String,
    pub operation_kind: String,
    pub state: String,
    pub trace_id: Option<String>,
    pub progress_payload: String,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub finished_at: Option<TimeDateTimeWithTimeZone>,
}

impl ActiveModelBehavior for ActiveModel {}
```

Replace `src-tauri/src/infra/sqlite/entities/mod.rs` with:

```rust
pub mod runtime_operations;
pub mod workspace_runtimes;
pub mod workspaces;
```

Delete:

```text
src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs
src-tauri/src/infra/sqlite/entities/runpod_runtime_operation_progress.rs
```

Do not add a migration: SeaORM's existing schema registry creates the new current schema for a fresh `db.sqlite`.

- [ ] **Step 5: Serialize and atomically save inline payloads**

In `src-tauri/src/adapters/sqlite/runtime_transition_repository.rs`, replace `use super::{runtime_operation_repository, runtime_persistence_dispatcher};` with:

```rust
use super::runtime_operation_repository;
```

Replace `save_transition` with:

```rust
#[diagnostic(show_error)]
async fn save_transition(
    &self,
    #[diagnostic(show)] workspace: &Workspace,
    #[diagnostic(show)] operation: &RuntimeOperation,
) -> Result<(), RuntimePersistenceError> {
    operation
        .validate_transition(workspace)
        .map_err(|_| RuntimePersistenceError::CorruptData)?;
    let provider_payload = workspace
        .runtime
        .as_ref()
        .map(|runtime| serde_json::to_string(&runtime.provider))
        .transpose()
        .map_err(|_| RuntimePersistenceError::CorruptData)?;
    let progress_payload = serde_json::to_string(&operation.progress)
        .map_err(|_| RuntimePersistenceError::CorruptData)?;

    let transaction = self
        .connection
        .begin()
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?;
    let operation_is_new = save_operation(operation, &progress_payload, &transaction).await?;

    match &workspace.runtime {
        Some(runtime) => {
            save_anchor(
                &workspace.id,
                runtime.state,
                provider_payload
                    .as_deref()
                    .ok_or(RuntimePersistenceError::CorruptData)?,
                operation,
                operation_is_new,
                &transaction,
            )
            .await?;
        }
        None => delete_anchor(&workspace.id, &transaction).await?,
    }

    transaction
        .commit()
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)
}
```

Replace `save_anchor`, `upsert_anchor`, `insert_anchor`, and `claim_cleanup_anchor` with:

```rust
async fn save_anchor(
    workspace_id: &str,
    state: RuntimeState,
    provider_payload: &str,
    operation: &RuntimeOperation,
    operation_is_new: bool,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    if operation_is_new && operation.state == RuntimeOperationState::Running {
        return match operation.kind {
            RuntimeOperationKind::Provision => insert_anchor(
                workspace_id,
                state,
                operation.runtime_kind,
                provider_payload,
                transaction,
            )
            .await
            .map_err(|error| match error {
                RuntimePersistenceError::AlreadyExists => {
                    RuntimePersistenceError::OperationAlreadyRunning
                }
                error => error,
            }),
            RuntimeOperationKind::Cleanup => {
                claim_cleanup_anchor(
                    workspace_id,
                    operation.runtime_kind,
                    provider_payload,
                    transaction,
                )
                .await
            }
        };
    }
    upsert_anchor(
        workspace_id,
        state,
        operation.runtime_kind,
        provider_payload,
        transaction,
    )
    .await
}

async fn upsert_anchor(
    workspace_id: &str,
    state: RuntimeState,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            if model
                .runtime_kind
                .parse::<RuntimeKind>()
                .map_err(|_| RuntimePersistenceError::CorruptData)?
                != kind
            {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.state = Set(runtime_state_value(state).to_owned());
            model.provider_payload = Set(provider_payload.to_owned());
            model
                .update(transaction)
                .await
                .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
        None => {
            insert_anchor(workspace_id, state, kind, provider_payload, transaction).await?;
        }
    }
    Ok(())
}

async fn insert_anchor(
    workspace_id: &str,
    state: RuntimeState,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    workspace_runtimes::ActiveModel {
        workspace_id: Set(workspace_id.to_owned()),
        runtime_kind: Set(kind.as_str().to_owned()),
        state: Set(runtime_state_value(state).to_owned()),
        provider_payload: Set(provider_payload.to_owned()),
    }
    .insert(transaction)
    .await
    .map_err(|error| match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => RuntimePersistenceError::AlreadyExists,
        _ => RuntimePersistenceError::Unavailable,
    })?;
    Ok(())
}

async fn claim_cleanup_anchor(
    workspace_id: &str,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    let result = workspace_runtimes::Entity::update_many()
        .col_expr(
            workspace_runtimes::Column::State,
            Expr::value("cleaning_up"),
        )
        .col_expr(
            workspace_runtimes::Column::ProviderPayload,
            Expr::value(provider_payload.to_owned()),
        )
        .filter(workspace_runtimes::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_runtimes::Column::RuntimeKind.eq(kind.as_str()))
        .filter(workspace_runtimes::Column::State.is_in(["ready", "failed"]))
        .exec(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?;
    if result.rows_affected == 1 {
        return Ok(());
    }

    match workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        None => Err(RuntimePersistenceError::NotFound),
        Some(model) if model.runtime_kind != kind.as_str() => {
            Err(RuntimePersistenceError::CorruptData)
        }
        Some(model) if matches!(model.state.as_str(), "provisioning" | "cleaning_up") => {
            Err(RuntimePersistenceError::OperationAlreadyRunning)
        }
        Some(_) => Err(RuntimePersistenceError::CorruptData),
    }
}
```

Replace `save_operation` with:

```rust
async fn save_operation(
    operation: &RuntimeOperation,
    progress_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<bool, RuntimePersistenceError> {
    match runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            if model.workspace_id != operation.workspace_id
                || model
                    .runtime_kind
                    .parse::<RuntimeKind>()
                    .map_err(|_| RuntimePersistenceError::CorruptData)?
                    != operation.runtime_kind
                || model.operation_kind
                    != runtime_operation_repository::runtime_operation_kind_value(operation.kind)
            {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.state = Set(runtime_operation_repository::runtime_operation_state_value(
                operation.state,
            )
            .to_owned());
            model.progress_payload = Set(progress_payload.to_owned());
            model.updated_at = Set(operation.updated_at);
            model.finished_at = Set(operation.finished_at);
            model
                .update(transaction)
                .await
                .map_err(map_operation_error)?;
            Ok(false)
        }
        None => {
            runtime_operations::ActiveModel {
                id: Set(operation.id.to_string()),
                workspace_id: Set(operation.workspace_id.clone()),
                runtime_kind: Set(operation.runtime_kind.as_str().to_owned()),
                operation_kind: Set(runtime_operation_repository::runtime_operation_kind_value(
                    operation.kind,
                )
                .to_owned()),
                state: Set(runtime_operation_repository::runtime_operation_state_value(
                    operation.state,
                )
                .to_owned()),
                trace_id: Set(operation.trace_id.map(|trace_id| trace_id.to_string())),
                progress_payload: Set(progress_payload.to_owned()),
                created_at: Set(operation.created_at),
                updated_at: Set(operation.updated_at),
                finished_at: Set(operation.finished_at),
            }
            .insert(transaction)
            .await
            .map_err(map_operation_error)?;
            Ok(true)
        }
    }
}
```

Keep `delete_anchor`, `runtime_state_value`, and `map_operation_error` unchanged.

- [ ] **Step 6: Hydrate workspace runtimes from the joined anchor row**

In `src-tauri/src/adapters/sqlite/workspace_repository.rs`, delete `use super::runtime_persistence_dispatcher;`.

Replace the runtime portion of `get` after the `else` block with:

```rust
let runtime = anchor.map(map_runtime).transpose()?;
Ok(Some(map_workspace(workspace, runtime)))
```

In `page`, replace everything from `let runtime_ids = rows` through the `workspaces` collection with:

```rust
let workspaces = rows
    .into_iter()
    .map(|(workspace, anchor)| {
        let runtime = anchor.map(map_runtime).transpose()?;
        Ok(map_workspace(workspace, runtime))
    })
    .collect::<Result<Vec<_>, _>>()?;
```

Replace `map_runtime`, `parse_runtime_kind`, and remove `map_runtime_error`:

```rust
fn map_runtime(anchor: workspace_runtimes::Model) -> Result<Runtime, WorkspaceRepositoryError> {
    let kind = parse_runtime_kind(&anchor.runtime_kind)?;
    let provider = serde_json::from_str::<RuntimeProvider>(&anchor.provider_payload)
        .map_err(|_| WorkspaceRepositoryError::CorruptData)?;
    if provider.kind() != kind {
        return Err(WorkspaceRepositoryError::CorruptData);
    }
    Ok(Runtime {
        state: parse_runtime_state(&anchor.state)?,
        provider,
    })
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, WorkspaceRepositoryError> {
    value
        .parse()
        .map_err(|_| WorkspaceRepositoryError::CorruptData)
}
```

Keep workspace creation/deletion, neutral state parsing, count, ordering, offset, and limit code unchanged.

- [ ] **Step 7: Hydrate operation progress from each selected operation row**

In `src-tauri/src/adapters/sqlite/runtime_operation_repository.rs`, delete `use super::runtime_persistence_dispatcher;` and replace `SqliteRuntimeOperationRepository::load` with:

```rust
async fn load(
    &self,
    query: sea_orm::Select<runtime_operations::Entity>,
) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
    query
        .all(&self.connection)
        .await
        .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
        .into_iter()
        .map(map_operation)
        .collect()
}
```

Replace `map_operation` and `parse_runtime_kind` with:

```rust
fn map_operation(
    model: runtime_operations::Model,
) -> Result<RuntimeOperation, RuntimeOperationRepositoryError> {
    let runtime_kind = parse_runtime_kind(&model.runtime_kind)?;
    let kind = match model.operation_kind.as_str() {
        "provision" => RuntimeOperationKind::Provision,
        "cleanup" => RuntimeOperationKind::Cleanup,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let state = match model.state.as_str() {
        "running" => RuntimeOperationState::Running,
        "succeeded" => RuntimeOperationState::Succeeded,
        "failed" => RuntimeOperationState::Failed,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let progress = serde_json::from_str::<RuntimeProgress>(&model.progress_payload)
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)?;
    let operation = RuntimeOperation {
        id: Uuid::parse_str(&model.id)
            .map_err(|_| RuntimeOperationRepositoryError::CorruptData)?,
        workspace_id: model.workspace_id,
        runtime_kind,
        kind,
        state,
        trace_id: model
            .trace_id
            .map(|trace_id| {
                Uuid::parse_str(&trace_id)
                    .map_err(|_| RuntimeOperationRepositoryError::CorruptData)
            })
            .transpose()?,
        progress,
        created_at: model.created_at,
        updated_at: model.updated_at,
        finished_at: model.finished_at,
    };
    operation
        .validate_progress()
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)?;
    Ok(operation)
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, RuntimeOperationRepositoryError> {
    value
        .parse()
        .map_err(|_| RuntimeOperationRepositoryError::CorruptData)
}
```

Replace the test helper and assertions in the existing unit-test module with:

```rust
fn model(trace_id: Option<&str>) -> runtime_operations::Model {
    runtime_operations::Model {
        id: Uuid::nil().to_string(),
        workspace_id: "workspace-1".into(),
        runtime_kind: RuntimeKind::Runpod.as_str().into(),
        operation_kind: "provision".into(),
        state: "running".into(),
        trace_id: trace_id.map(str::to_owned),
        progress_payload: serde_json::to_string(
            &crate::application::runtimes::progress_fixture(),
        )
        .unwrap(),
        created_at: time::OffsetDateTime::UNIX_EPOCH,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
        finished_at: None,
    }
}

#[test]
fn trace_mapping_accepts_uuid_or_null_and_rejects_invalid_text() {
    let trace_id = Uuid::new_v4();
    assert_eq!(
        map_operation(model(Some(&trace_id.to_string())))
            .unwrap()
            .trace_id,
        Some(trace_id)
    );
    assert_eq!(map_operation(model(None)).unwrap().trace_id, None);
    assert_eq!(
        map_operation(model(Some("invalid"))),
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
}
```

Keep the page count/filter/order/offset/limit logic, `running`, `has_running`, and neutral operation/state conversion functions unchanged.

- [ ] **Step 8: Remove SQLite provider persistence and dispatch modules**

Replace `src-tauri/src/adapters/sqlite/mod.rs` with:

```rust
mod runtime_operation_repository;
mod runtime_transition_repository;
mod workspace_repository;

pub use runtime_operation_repository::SqliteRuntimeOperationRepository;
pub use runtime_transition_repository::SqliteRuntimeTransitionRepository;
pub use workspace_repository::SqliteWorkspaceRepository;
```

Delete:

```text
src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs
src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs
```

- [ ] **Step 9: Run focused persistence tests and removal checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test sqlite_runtime_payload_persistence
cargo test --manifest-path src-tauri/Cargo.toml --lib trace_mapping_accepts_uuid_or_null_and_rejects_invalid_text
rg -n "runtime_persistence_dispatcher|runpod_runtime_persistence|runpod_workspace_runtimes|runpod_runtime_operation_progress" src-tauri/src src-tauri/tests
```

Expected: both test commands pass; the final search prints nothing and exits with status 1.

- [ ] **Step 10: Verify payloads remain private and public contracts remain unchanged**

Run:

```bash
rg -n "provider_payload|progress_payload" src-tauri/src/facade src-tauri/src/diagnostics src/generated/commands.ts
git diff --exit-code -- src/generated/commands.ts
```

Expected: the search prints nothing and exits with status 1; generated command bindings have no diff and the second command exits 0. Review the Task 1 payload structs once more and confirm their only fields are RunPod configuration/resource IDs and operation progress, with no credential-bearing field.

- [ ] **Step 11: Run full native verification**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all tests pass, formatting is clean, and Clippy reports no warnings.

- [ ] **Step 12: Inspect scope and commit the SQLite cutover**

Run:

```bash
git status --short
git diff --stat HEAD
```

Expected: only the Task 2 entity, adapter, and renamed integration-test paths are changed since the Task 1 commit; no port, facade, diagnostics, generated, frontend, migration, or dependency file appears.

Commit:

```bash
git add src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs src-tauri/src/infra/sqlite/entities/runtime_operations.rs src-tauri/src/infra/sqlite/entities/mod.rs src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs src-tauri/src/infra/sqlite/entities/runpod_runtime_operation_progress.rs src-tauri/src/adapters/sqlite/runtime_transition_repository.rs src-tauri/src/adapters/sqlite/workspace_repository.rs src-tauri/src/adapters/sqlite/runtime_operation_repository.rs src-tauri/src/adapters/sqlite/mod.rs src-tauri/src/adapters/sqlite/runpod_runtime_persistence.rs src-tauri/src/adapters/sqlite/runtime_persistence_dispatcher.rs src-tauri/tests/sqlite_runtime_dispatch.rs src-tauri/tests/sqlite_runtime_payload_persistence.rs
git commit -m "refactor(sqlite): persist runtime provider payloads inline"
```

The completed cutover deliberately adds no generic payload repository, provider persistence trait, JSON wrapper type, migration, or query instrumentation. The existing two repository reads and one transaction boundary already cover the approved contract.
