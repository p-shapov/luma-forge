# Application Ports and Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the provider-neutral application layer, RunPod runtime workflows, lifecycle journal contracts, secret workflows, and application-facing adapters over the existing raw infra primitives in `src-tauri/src`.

**Architecture:** `application` owns models, behavior, ports, and typed errors; `infra` remains application-agnostic; `adapters` translate between them. RunPod Provision/Cleanup own provider-specific sequencing, while lifecycle operations remain a provider-neutral journal with typed RunPod progress. Runtime and lifecycle changes are persisted through one atomic `RunpodRuntimeRepository::save_transition` boundary.

**Tech Stack:** Rust 2021, Tokio, `async-trait`, `secrecy`, `time`, `uuid`, SeaORM `2.0.0-rc.41`, SQLite, existing bundled JSON-schema codegen, existing reqwest RunPod/Hugging Face clients.

**Design:** `docs/2026-07-11-application-ports-adapters-design.md`

## Global Constraints

- Work only in the active `src-tauri/src` tree. Do not read from, import, compile, copy, or commit `src-tauri/old_src`; Git history is the canonical reference for removed code.
- The source-tree cutover is already complete. Do not add cutover, migration, legacy fallback, compatibility shim, or deprecated contract work.
- Keep `src-tauri/src/lib.rs` minimal and export only `application`, `adapters`, and `infra` as those modules land.
- `application` must not import `crate::infra`, SeaORM, reqwest, keyring, Tauri, Specta, or generated transport/catalog types.
- `infra` must not import `crate::application` or `crate::adapters`.
- Raw credentials remain `SecretString`; never include them in `Debug`, snapshots, errors, logs, fixtures, generated types, or persisted SQLite rows.
- Do not run Tauri command codegen. Do not update generated frontend contracts. Frontend build/runtime failure is expected and must not be repaired.
- Add behavioral tests only under `application`; do not add adapter, infra, SQLite integration, Tauri, Specta, generated-contract, or frontend tests.
- Do not implement reconciliation/resume, cursor pagination, multiple runtimes per workspace, secret overwrite, a second compute provider, retry frameworks, or a generic unit-of-work abstraction.
- Application test commands use focused `cargo test --manifest-path src-tauri/Cargo.toml <test-path> -- --exact` RED/GREEN cycles.
- Each task ends with `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and a focused/full native test gate appropriate to that task.
- Commit only the files listed in the task. Use Conventional Commits.

---

## File Structure

### Application

- `src-tauri/src/application/mod.rs`: top-level application exports.
- `src-tauri/src/application/catalog.rs`: shared catalog references and runtime definition values.
- `src-tauri/src/application/lifecycle/{mod.rs,model.rs,errors.rs}`: lifecycle operation aggregate and state transitions.
- `src-tauri/src/application/lifecycle/progress/{mod.rs,runpod.rs}`: typed RunPod Provision/Cleanup progress.
- `src-tauri/src/application/lifecycle/ports/{mod.rs,lifecycle_operation_repository.rs}`: provider-neutral journal reads.
- `src-tauri/src/application/workspace/{mod.rs,model.rs,errors.rs,service.rs}`: immutable workspace and create/delete behavior.
- `src-tauri/src/application/workspace/ports/{mod.rs,workflow_catalog.rs,workspace_repository.rs}`: workspace-driven ports.
- `src-tauri/src/application/secrets/{mod.rs,model.rs,errors.rs,service.rs}`: secret status, identity, and management behavior.
- `src-tauri/src/application/secrets/ports/{mod.rs,identity_provider.rs,secret_store.rs}`: typed secret ports.
- `src-tauri/src/application/runtimes/{mod.rs,runpod/mod.rs}`: runtime exports.
- `src-tauri/src/application/runtimes/runpod/{model.rs,errors.rs,service.rs,test_support.rs}`: RunPod state machine and workflows.
- `src-tauri/src/application/runtimes/runpod/ports/{mod.rs,runtime_catalog.rs,runtime_provider.rs,runtime_repository.rs}`: RunPod-specific ports.

### Adapters

- `src-tauri/src/adapters/mod.rs`: adapter exports.
- `src-tauri/src/adapters/bundled/{mod.rs,workflow_catalog.rs,runpod_runtime_catalog.rs}`: bundled entry mappings.
- `src-tauri/src/adapters/keyring/{mod.rs,secret_store.rs}`: `SecretKind` to account-name mapping and keyring semantics.
- `src-tauri/src/adapters/runpod/{mod.rs,identity_provider.rs,runtime_provider.rs}`: RunPod client mappings.
- `src-tauri/src/adapters/hugging_face/{mod.rs,identity_provider.rs}`: Hugging Face identity mapping.
- `src-tauri/src/adapters/sqlite/{mod.rs,workspace_repository.rs,lifecycle_operation_repository.rs,runpod_runtime_repository.rs}`: SeaORM mappings and transactions.

### Existing Infra and Bundled Data

- `new_bundled/catalog/schemas/workflow_metadata`: add required description.
- `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata`: provide description.
- `src-tauri/src/infra/clients/{errors.rs,http.rs}`: preserve HTTP NotFound for idempotent cleanup mapping.
- `src-tauri/src/infra/clients/runpod/client.rs`: add one raw provisioner status request.
- `src-tauri/src/infra/sqlite/entities/{mod.rs,workspaces.rs,workspace_runtimes.rs,runpod_workspace_runtimes.rs,lifecycle_operations.rs,runpod_lifecycle_progress.rs}`: current normalized persistence shape.

## Spec Coverage

- Workspace immutability, workflow validation, deletion guards, and attached runtime kind: Tasks 1 and 3.
- One runtime per workspace, RunPod state/config/resources, atomic persistence, and workspace status projection: Tasks 5, 6, 7, and 10.
- Lifecycle state/progress, one-running invariant, history reads, trace retention, and interrupted startup behavior: Tasks 2, 6, 7, and 10.
- Flat workflow summaries, description/requirements fields, exact workflow lookup, and resolved RunPod definitions: Tasks 1, 6, and 8.
- Validate-before-insert secrets, common Identity, explicit status/identity/delete, and account-name ownership: Tasks 4 and 8.
- RunPod raw client translation, credential-safe provider calls, idempotent cleanup, and polling: Task 9.
- Layer dependency rules, application-only behavioral tests, and explicit Tauri/frontend exclusions: Task 11.

No approved spec requirement is intentionally omitted.

---

### Task 1: Application Catalog Contracts and Workflow Metadata

**Files:**
- Create: `src-tauri/src/application/mod.rs`
- Create: `src-tauri/src/application/catalog.rs`
- Create: `src-tauri/src/application/workspace/mod.rs`
- Create: `src-tauri/src/application/workspace/model.rs`
- Create: `src-tauri/src/application/workspace/ports/mod.rs`
- Create: `src-tauri/src/application/workspace/ports/workflow_catalog.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `new_bundled/catalog/schemas/workflow_metadata`
- Modify: `new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata`

**Interfaces:**
- Consumes: existing `async-trait`, `serde_json::Value`, `time::OffsetDateTime`.
- Produces: `CatalogRef`, `RunpodContractRequirements`, `RuntimeContractRequirements`, `WorkflowSummary`, `WorkflowDefinition`, `RuntimePreset`, `RuntimeContract`, `RunpodRuntimeDefinition`, `WorkflowCatalog`, and immutable `Workspace`/`RuntimeKind` models.

- [ ] **Step 1: Add the failing application catalog behavior test**

Create `src-tauri/src/application/catalog.rs` with the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_runpod_requirements_without_erasing_the_provider_type() {
        let requirements = vec![RuntimeContractRequirements::Runpod(
            RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
            },
        )];

        assert_eq!(
            WorkflowDefinition::runpod_requirements(&requirements),
            Some(&RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
            })
        );
    }
}
```

Create the module shells:

```rust
// src-tauri/src/application/mod.rs
pub mod catalog;
pub mod workspace;

// src-tauri/src/application/workspace/mod.rs
mod model;
pub mod ports;

pub use model::{RuntimeKind, Workspace, WorkspaceStatus};

// src-tauri/src/application/workspace/ports/mod.rs
mod workflow_catalog;
pub use workflow_catalog::{WorkflowCatalog, WorkflowCatalogError};
```

Add `pub mod application;` before `pub mod infra;` in `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::catalog::tests::selects_runpod_requirements_without_erasing_the_provider_type -- --exact
```

Expected: compilation fails because the catalog types and `WorkflowDefinition::runpod_requirements` do not exist.

- [ ] **Step 3: Implement the minimal catalog and workspace types**

Add these exact public shapes to `application/catalog.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRef {
    pub id: String,
    pub revision: String,
}

impl CatalogRef {
    pub fn new(id: impl Into<String>, revision: impl Into<String>) -> Self {
        Self { id: id.into(), revision: revision.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodContractRequirements {
    pub provisioner_contract_ref: CatalogRef,
    pub endpoint_contract_ref: CatalogRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeContractRequirements {
    Runpod(RunpodContractRequirements),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    pub summary: WorkflowSummary,
    pub runtime_preset_ref: CatalogRef,
    pub contract_requirements: Vec<RuntimeContractRequirements>,
    pub model_assets: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub workflow_graph: serde_json::Value,
}

impl WorkflowDefinition {
    pub fn runpod_requirements(
        requirements: &[RuntimeContractRequirements],
    ) -> Option<&RunpodContractRequirements> {
        requirements.iter().find_map(|value| match value {
            RuntimeContractRequirements::Runpod(value) => Some(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePreset(pub serde_json::Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeContract {
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunpodRuntimeDefinition {
    pub runtime_preset: RuntimePreset,
    pub provisioner_contract: RuntimeContract,
    pub endpoint_contract: RuntimeContract,
}
```

Create `application/workspace/model.rs`:

```rust
use time::OffsetDateTime;

use crate::application::catalog::CatalogRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind { Runpod }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStatus { NotProvisioned, Provisioning, Ready, CleaningUp, Failed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub workflow: CatalogRef,
    pub created_at: OffsetDateTime,
    pub attached_runtime: Option<RuntimeKind>,
}
```

Create `application/workspace/ports/workflow_catalog.rs`:

```rust
use async_trait::async_trait;

use crate::application::catalog::{WorkflowDefinition, WorkflowSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowCatalogError {
    #[error("bundled catalog is invalid")]
    InvalidCatalog,
    #[error("bundled catalog is unavailable")]
    Unavailable,
}

#[async_trait]
pub trait WorkflowCatalog: Send + Sync {
    async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError>;
    async fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError>;
}
```

- [ ] **Step 4: Add description to the current bundled contract and entry**

In `new_bundled/catalog/schemas/workflow_metadata`, add `description` to `required` and add:

```json
"description": { "type": "string", "minLength": 1 }
```

In the current workflow metadata entry add:

```json
"description": "Generate images with the ComfyUI HiDream O1 Dev workflow."
```

Do not edit `src-tauri/src/infra/bundled/generated.rs`; `build.rs` regenerates the OUT_DIR type.

- [ ] **Step 5: Run GREEN and compile generated catalog types**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::catalog::tests::selects_runpod_requirements_without_erasing_the_provider_type -- --exact
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: the focused test passes; catalog codegen accepts the new required field; check and formatting exit 0.

- [ ] **Step 6: Commit**

```bash
git add new_bundled/catalog/schemas/workflow_metadata new_bundled/catalog/entries/workflows/comfyui-hidream-o1-dev/1.0.0/metadata src-tauri/src/lib.rs src-tauri/src/application
git commit -m "feat(application): add catalog contracts"
```

---

### Task 2: Lifecycle Operation Aggregate and Journal Port

**Files:**
- Create: `src-tauri/src/application/lifecycle/mod.rs`
- Create: `src-tauri/src/application/lifecycle/model.rs`
- Create: `src-tauri/src/application/lifecycle/errors.rs`
- Create: `src-tauri/src/application/lifecycle/progress/mod.rs`
- Create: `src-tauri/src/application/lifecycle/progress/runpod.rs`
- Create: `src-tauri/src/application/lifecycle/ports/mod.rs`
- Create: `src-tauri/src/application/lifecycle/ports/lifecycle_operation_repository.rs`
- Modify: `src-tauri/src/application/mod.rs`

**Interfaces:**
- Consumes: `time::OffsetDateTime` and `uuid::Uuid`.
- Produces: typed RunPod steps/progress, `LifecycleOperation`, `LifecycleOperationState`, transition errors, and read-only `LifecycleOperationRepository`.

- [ ] **Step 1: Write failing lifecycle transition tests**

Add to `application/lifecycle/model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::lifecycle::progress::runpod::{
        RunpodCleanupStep, RunpodProvisionStep,
    };

    #[test]
    fn running_operation_can_succeed_once_and_retains_its_step() {
        let mut operation = LifecycleOperation::runpod_provision(
            "operation-1",
            "workspace-1",
            "trace-1",
            RunpodProvisionStep::CreateNetworkVolume,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, LifecycleOperationState::Succeeded);
        assert_eq!(operation.progress.provision_step(), Some(RunpodProvisionStep::CreateNetworkVolume));
        assert_eq!(operation.succeed(OffsetDateTime::UNIX_EPOCH), Err(LifecycleError::InvalidTransition));
    }

    #[test]
    fn interrupted_operation_fails_without_changing_progress_or_trace() {
        let mut operation = LifecycleOperation::runpod_cleanup(
            "operation-1",
            "workspace-1",
            "trace-1",
            RunpodCleanupStep::DeleteEndpoint,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.fail(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, LifecycleOperationState::Failed);
        assert_eq!(operation.trace_id, "trace-1");
        assert_eq!(operation.progress.cleanup_step(), Some(RunpodCleanupStep::DeleteEndpoint));
    }
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::lifecycle::model::tests::running_operation_can_succeed_once_and_retains_its_step -- --exact
```

Expected: compilation fails because lifecycle modules and types do not exist.

- [ ] **Step 3: Implement typed progress and minimal state transitions**

Define in `progress/runpod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProvisionStep {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProgress {
    Provision(RunpodProvisionStep),
    Cleanup(RunpodCleanupStep),
}
```

Define in `model.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperationState { Running, Succeeded, Failed }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperationKind { Provision, Cleanup }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleProgress { Runpod(RunpodProgress) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOperation {
    pub id: String,
    pub workspace_id: String,
    pub state: LifecycleOperationState,
    pub trace_id: String,
    pub progress: LifecycleProgress,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}
```

Implement `runpod_provision`, `runpod_cleanup`, `set_provision_step`, `set_cleanup_step`, `succeed`, `fail`, and `kind`. `set_*` and terminal transitions return `LifecycleError::InvalidTransition` unless state is `Running`. Add progress accessors used by the tests. Do not add a generic payload map.

Define `LifecycleError` exactly as:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("lifecycle operation transition is invalid")]
    InvalidTransition,
    #[error("workspace already has a running lifecycle operation")]
    OperationAlreadyRunning,
}
```

- [ ] **Step 4: Add the journal read port**

Create `ports/lifecycle_operation_repository.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleOperationRepositoryError {
    #[error("lifecycle journal is unavailable")]
    Unavailable,
    #[error("lifecycle journal contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait LifecycleOperationRepository: Send + Sync {
    async fn recent(&self, limit: u64) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn recent_for_workspace(&self, workspace_id: &str, limit: u64) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn running(&self) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError>;
    async fn has_running(&self, workspace_id: &str) -> Result<bool, LifecycleOperationRepositoryError>;
}
```

Export the modules from `application/lifecycle/mod.rs` and add `pub mod lifecycle;` to `application/mod.rs`.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::lifecycle::model::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: both lifecycle behavior tests pass; formatting exits 0.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/mod.rs src-tauri/src/application/lifecycle
git commit -m "feat(lifecycle): add operation journal model"
```

---

### Task 3: Immutable Workspace Service and Repository Port

**Files:**
- Create: `src-tauri/src/application/workspace/errors.rs`
- Create: `src-tauri/src/application/workspace/service.rs`
- Create: `src-tauri/src/application/workspace/ports/workspace_repository.rs`
- Modify: `src-tauri/src/application/workspace/mod.rs`
- Modify: `src-tauri/src/application/workspace/ports/mod.rs`

**Interfaces:**
- Consumes: `WorkflowCatalog`, `LifecycleOperationRepository`, `CatalogRef`, and `Workspace` from Tasks 1-2.
- Produces: `WorkspaceRepository` and `WorkspaceService::{create,delete,get,list}` with immutable workflow references and deletion guards.

- [ ] **Step 1: Write failing workspace behavioral tests with local fakes**

In `workspace/service.rs`, add tests covering these exact cases:

```rust
#[tokio::test]
async fn create_rejects_an_unknown_workflow_without_writing() {
    let fakes = Fakes::with_missing_workflow();
    let service = fakes.service();

    let result = service.create("workspace-1", CatalogRef::new("missing", "1.0.0")).await;

    assert_eq!(result, Err(WorkspaceError::WorkflowNotFound));
    assert!(fakes.workspaces.created().is_empty());
}

#[tokio::test]
async fn delete_rejects_an_attached_runtime() {
    let fakes = Fakes::with_workspace(Workspace {
        id: "workspace-1".into(),
        workflow: CatalogRef::new("workflow", "1.0.0"),
        created_at: OffsetDateTime::UNIX_EPOCH,
        attached_runtime: Some(RuntimeKind::Runpod),
    });

    assert_eq!(fakes.service().delete("workspace-1").await, Err(WorkspaceError::RuntimeAttached));
}

#[tokio::test]
async fn delete_rejects_a_running_operation_and_preserves_history() {
    let fakes = Fakes::with_unprovisioned_workspace_and_running_operation();

    assert_eq!(fakes.service().delete("workspace-1").await, Err(WorkspaceError::OperationRunning));
    assert!(fakes.workspaces.contains("workspace-1"));
}
```

Implement test-only `FakeWorkspaceRepository`, `FakeWorkflowCatalog`, and `FakeLifecycleOperationRepository` inside the same `#[cfg(test)]` module. Store calls in `Mutex<Vec<_>>`; never create a reusable production fake abstraction.

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::workspace::service::tests::create_rejects_an_unknown_workflow_without_writing -- --exact
```

Expected: compilation fails because `WorkspaceService`, `WorkspaceRepository`, and `WorkspaceError` do not exist.

- [ ] **Step 3: Implement the exact workspace port and errors**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceRepositoryError {
    #[error("workspace already exists")]
    AlreadyExists,
    #[error("workspace persistence is unavailable")]
    Unavailable,
    #[error("workspace persistence contains invalid data")]
    CorruptData,
}

#[async_trait::async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn create(&self, workspace: Workspace) -> Result<Workspace, WorkspaceRepositoryError>;
    async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError>;
    async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError>;
    async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError>;
}
```

Define `WorkspaceError` with `NotFound`, `AlreadyExists`, `WorkflowNotFound`, `RuntimeAttached`, `OperationRunning`, `CatalogUnavailable`, and `PersistenceUnavailable`. Do not store raw catalog/database error strings.

- [ ] **Step 4: Implement minimal service behavior**

Use this dependency shape:

```rust
pub struct WorkspaceService<'a> {
    workspaces: &'a dyn WorkspaceRepository,
    lifecycle: &'a dyn LifecycleOperationRepository,
    workflows: &'a dyn WorkflowCatalog,
}
```

Implement:

```rust
pub async fn create(&self, id: &str, workflow: CatalogRef) -> Result<Workspace, WorkspaceError>
pub async fn delete(&self, id: &str) -> Result<(), WorkspaceError>
pub async fn get(&self, id: &str) -> Result<Workspace, WorkspaceError>
pub async fn list(&self) -> Result<Vec<Workspace>, WorkspaceError>
```

`create` checks `WorkflowCatalog::get` before writing and constructs `Workspace` with `OffsetDateTime::now_utc()` and no attached runtime. `delete` loads the workspace, rejects attached runtime, rejects `has_running`, then requires repository `delete` to return true. Map only typed errors.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::workspace::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all three workspace behavior tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/workspace
git commit -m "feat(workspace): add immutable workspace service"
```

---

### Task 4: Secret Management and Identity Behavior

**Files:**
- Create: `src-tauri/src/application/secrets/mod.rs`
- Create: `src-tauri/src/application/secrets/model.rs`
- Create: `src-tauri/src/application/secrets/errors.rs`
- Create: `src-tauri/src/application/secrets/service.rs`
- Create: `src-tauri/src/application/secrets/ports/mod.rs`
- Create: `src-tauri/src/application/secrets/ports/identity_provider.rs`
- Create: `src-tauri/src/application/secrets/ports/secret_store.rs`
- Modify: `src-tauri/src/application/mod.rs`

**Interfaces:**
- Consumes: `secrecy::SecretString`.
- Produces: `SecretKind`, `SecretStatus`, common `Identity`, `SecretIdentityProvider`, `SecretStore`, and `SecretsService`.

- [ ] **Step 1: Write failing secret behavior tests**

Add exact tests in `secrets/service.rs`:

```rust
#[tokio::test]
async fn set_validates_before_inserting() {
    let fakes = Fakes::empty();
    let identity = fakes.service().set(SecretKind::RunpodApiKey, SecretString::from("candidate")).await.unwrap();

    assert_eq!(identity.email.as_deref(), Some("user@example.com"));
    assert_eq!(fakes.calls(), vec!["exists:runpod", "identity:runpod", "insert:runpod"]);
}

#[tokio::test]
async fn set_rejects_an_existing_key_without_network_validation() {
    let fakes = Fakes::configured(SecretKind::RunpodApiKey);

    assert_eq!(fakes.service().set(SecretKind::RunpodApiKey, SecretString::from("candidate")).await, Err(SecretsError::AlreadyConfigured));
    assert_eq!(fakes.calls(), vec!["exists:runpod"]);
}

#[tokio::test]
async fn delete_missing_key_is_an_explicit_error() {
    let fakes = Fakes::empty();

    assert_eq!(fakes.service().delete(SecretKind::HuggingFaceApiKey).await, Err(SecretsError::NotConfigured));
}

#[tokio::test]
async fn status_does_not_read_the_raw_secret_or_call_the_network() {
    let fakes = Fakes::configured(SecretKind::RunpodApiKey);

    assert_eq!(fakes.service().status(SecretKind::RunpodApiKey).await.unwrap(), SecretStatus::Configured);
    assert_eq!(fakes.calls(), vec!["exists:runpod"]);
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::secrets::service::tests::set_validates_before_inserting -- --exact
```

Expected: compilation fails because the secrets application module does not exist.

- [ ] **Step 3: Implement models, ports, and typed errors**

Use these exact shapes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind { RunpodApiKey, HuggingFaceApiKey }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStatus { Missing, Configured }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub key_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
}

#[async_trait::async_trait]
pub trait SecretIdentityProvider: Send + Sync {
    async fn identity(&self, credential: &SecretString) -> Result<Identity, SecretIdentityProviderError>;
}

#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    async fn exists(&self, kind: SecretKind) -> Result<bool, SecretStoreError>;
    async fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretStoreError>;
    async fn insert(&self, kind: SecretKind, secret: SecretString) -> Result<(), SecretStoreError>;
    async fn delete(&self, kind: SecretKind) -> Result<(), SecretStoreError>;
}
```

`SecretStoreError` is `AlreadyExists | NotFound | Unavailable`; `SecretIdentityProviderError` is `InvalidCredential | Unavailable`; `SecretsError` is `AlreadyConfigured | NotConfigured | InvalidCredential | IdentityUnavailable | StorageUnavailable`.

- [ ] **Step 4: Implement the minimal service**

Use two implementations of the same identity trait:

```rust
pub struct SecretsService<'a> {
    store: &'a dyn SecretStore,
    runpod_identity: &'a dyn SecretIdentityProvider,
    hugging_face_identity: &'a dyn SecretIdentityProvider,
}
```

Implement `set`, `status`, `identity`, and `delete` exactly in the order asserted by the tests. `identity` calls `get` then the selected live provider. Do not cache Identity and do not add update/replace.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::secrets::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all four secret behavior tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/mod.rs src-tauri/src/application/secrets
git commit -m "feat(secrets): add credential workflows"
```

---

### Task 5: RunPod Runtime State and Ports

**Files:**
- Create: `src-tauri/src/application/runtimes/mod.rs`
- Create: `src-tauri/src/application/runtimes/runpod/mod.rs`
- Create: `src-tauri/src/application/runtimes/runpod/model.rs`
- Create: `src-tauri/src/application/runtimes/runpod/errors.rs`
- Create: `src-tauri/src/application/runtimes/runpod/ports/mod.rs`
- Create: `src-tauri/src/application/runtimes/runpod/ports/runtime_catalog.rs`
- Create: `src-tauri/src/application/runtimes/runpod/ports/runtime_provider.rs`
- Create: `src-tauri/src/application/runtimes/runpod/ports/runtime_repository.rs`
- Modify: `src-tauri/src/application/mod.rs`

**Interfaces:**
- Consumes: `RunpodRuntimeDefinition`, `RunpodContractRequirements`, `LifecycleOperation`, and `SecretString`.
- Produces: RunPod state/config/resources, exact provider commands, and repository/catalog/provider ports used by Tasks 6-9.

- [ ] **Step 1: Write failing runtime state tests**

```rust
#[test]
fn ready_runtime_can_start_cleanup_but_cannot_provision_again() {
    let mut runtime = runtime_in(RunpodRuntimeState::Ready);

    assert_eq!(runtime.begin_provision(), Err(RunpodRuntimeError::AlreadyProvisioned));
    runtime.begin_cleanup().unwrap();
    assert_eq!(runtime.state, RunpodRuntimeState::CleaningUp);
}

#[test]
fn failed_runtime_requires_cleanup() {
    let mut runtime = runtime_in(RunpodRuntimeState::Failed);

    assert_eq!(runtime.begin_provision(), Err(RunpodRuntimeError::RuntimeFailed));
    assert_eq!(runtime.begin_cleanup(), Ok(()));
}

#[test]
fn active_transition_rejects_another_operation() {
    for state in [RunpodRuntimeState::Provisioning, RunpodRuntimeState::CleaningUp] {
        let mut runtime = runtime_in(state);
        assert_eq!(runtime.begin_provision(), Err(RunpodRuntimeError::OperationInProgress));
    }
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::model::tests::ready_runtime_can_start_cleanup_but_cannot_provision_again -- --exact
```

Expected: compilation fails because RunPod application types do not exist.

- [ ] **Step 3: Implement the state model**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodRuntimeState { Provisioning, Ready, CleaningUp, Failed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeConfig {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunpodRuntimeResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub template_id: Option<String>,
    pub endpoint_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntime {
    pub workspace_id: String,
    pub state: RunpodRuntimeState,
    pub config: RunpodRuntimeConfig,
    pub resources: RunpodRuntimeResources,
}
```

Implement `new_provisioning`, `begin_provision`, `begin_cleanup`, `mark_ready`, and `mark_failed` with only the approved transitions. Add `impl From<RunpodRuntimeState> for WorkspaceStatus`.

- [ ] **Step 4: Define exact RunPod ports**

Provider commands:

```rust
pub struct CreateNetworkVolume {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub size_gb: u64,
}

pub struct StartProvisionerPod {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub required_model_assets: serde_json::Value,
    pub hugging_face_api_key: Option<SecretString>,
}

pub struct CreateTemplate { pub workspace_id: String, pub image_ref: String }

pub struct CreateEndpoint {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub network_volume_id: String,
    pub template_id: String,
}
```

Provider port:

```rust
#[async_trait::async_trait]
pub trait RunpodRuntimeProvider: Send + Sync {
    async fn create_network_volume(&self, api_key: &SecretString, command: CreateNetworkVolume) -> Result<String, RunpodRuntimeProviderError>;
    async fn start_provisioner_pod(&self, api_key: &SecretString, command: StartProvisionerPod) -> Result<String, RunpodRuntimeProviderError>;
    async fn wait_for_provisioner(&self, api_key: &SecretString, workspace_id: &str, pod_id: &str) -> Result<(), RunpodRuntimeProviderError>;
    async fn terminate_provisioner_pod(&self, api_key: &SecretString, pod_id: &str) -> Result<(), RunpodRuntimeProviderError>;
    async fn create_template(&self, api_key: &SecretString, command: CreateTemplate) -> Result<String, RunpodRuntimeProviderError>;
    async fn create_endpoint(&self, api_key: &SecretString, command: CreateEndpoint) -> Result<String, RunpodRuntimeProviderError>;
    async fn delete_endpoint(&self, api_key: &SecretString, id: &str) -> Result<(), RunpodRuntimeProviderError>;
    async fn delete_template(&self, api_key: &SecretString, id: &str) -> Result<(), RunpodRuntimeProviderError>;
    async fn delete_network_volume(&self, api_key: &SecretString, id: &str) -> Result<(), RunpodRuntimeProviderError>;
}
```

`RunpodRuntimeProviderError` is `Unauthorized | Unavailable | ProvisionerFailed` and contains no provider message.

Catalog and repository ports:

```rust
#[async_trait::async_trait]
pub trait RunpodRuntimeCatalog: Send + Sync {
    async fn resolve(&self, preset: &CatalogRef, requirements: &RunpodContractRequirements) -> Result<RunpodRuntimeDefinition, RunpodRuntimeCatalogError>;
}

#[async_trait::async_trait]
pub trait RunpodRuntimeRepository: Send + Sync {
    async fn get(&self, workspace_id: &str) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError>;
    async fn save_transition(&self, runtime: &RunpodRuntime, operation: &LifecycleOperation) -> Result<(), RunpodRuntimeRepositoryError>;
}
```

Repository errors: `AlreadyExists | OperationAlreadyRunning | NotFound | Unavailable | CorruptData`. Catalog errors: `InvalidCatalog | Unavailable`.

Define the service/state error enum exactly as:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeError {
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("workflow was not found")]
    WorkflowNotFound,
    #[error("runtime is already provisioned")]
    AlreadyProvisioned,
    #[error("runtime is failed and must be cleaned up")]
    RuntimeFailed,
    #[error("runtime operation is already in progress")]
    OperationInProgress,
    #[error("runtime is not provisioned")]
    NotProvisioned,
    #[error("required credential is not configured")]
    CredentialMissing,
    #[error("runtime provider is unavailable")]
    ProviderUnavailable,
    #[error("application catalog is unavailable or invalid")]
    CatalogUnavailable,
    #[error("runtime persistence is unavailable or invalid")]
    PersistenceUnavailable,
    #[error("runtime transition is invalid")]
    InvalidTransition,
}
```

Map `RunpodRuntimeProviderError::{Unauthorized,Unavailable,ProvisionerFailed}` to `ProviderUnavailable`; catalog errors to `CatalogUnavailable`; repository, lifecycle-repository, and secret-storage failures to `PersistenceUnavailable`; missing required credentials to `CredentialMissing`; lifecycle transition failures to `InvalidTransition`. Do not carry raw source messages.

- [ ] **Step 5: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::model::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: all three state tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/application/mod.rs src-tauri/src/application/runtimes
git commit -m "feat(runpod): add runtime state and ports"
```

---

### Task 6: RunPod Provision Workflow

**Files:**
- Create: `src-tauri/src/application/runtimes/runpod/service.rs`
- Create: `src-tauri/src/application/runtimes/runpod/test_support.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/mod.rs`

**Interfaces:**
- Consumes: every RunPod port from Task 5, `WorkspaceRepository`, `WorkflowCatalog`, `LifecycleOperationRepository`, and `SecretStore`.
- Produces: `RunpodRuntimeService::provision` with the approved six-step order and atomic transition writes.

- [ ] **Step 1: Build test-only fakes and write the failing happy-path test**

`test_support.rs` must provide in-memory fakes for all consumed ports. `FakeRunpodRuntimeProvider` records provider method names and returns IDs `volume-1`, `pod-1`, `template-1`, and `endpoint-1`. `FakeRunpodRuntimeRepository` records cloned `(RunpodRuntime, LifecycleOperation)` snapshots.

Expose this test-only harness API so Tasks 6-7 use one consistent fake set:

```rust
pub(super) struct ProvisionFakes {
    pub provider: FakeRunpodRuntimeProvider,
    pub repository: FakeRunpodRuntimeRepository,
    workspaces: FakeWorkspaceRepository,
    workflows: FakeWorkflowCatalog,
    runtime_catalog: FakeRunpodRuntimeCatalog,
    lifecycle: FakeLifecycleOperationRepository,
    secrets: FakeSecretStore,
}

pub(super) type CleanupFakes = ProvisionFakes;
pub(super) type RecoveryFakes = ProvisionFakes;

impl ProvisionFakes {
    pub fn ready() -> Self;
    pub fn ready_runtime() -> Self;
    pub fn failed_partial_runtime() -> Self;
    pub fn without_runtime() -> Self;
    pub fn with_running_provision_and_cleanup() -> Self;
    pub fn service(&self) -> RunpodRuntimeService<'_>;
}
```

These declarations describe test-only constructors; implement each constructor with concrete in-memory values and `std::sync::Mutex` call/snapshot vectors in the same file. Do not add a mocking dependency.

Add this test in `service.rs`:

```rust
#[tokio::test]
async fn provision_persists_each_current_step_before_the_provider_call() {
    let fakes = ProvisionFakes::ready();

    let runtime = fakes.service().provision(ProvisionRunpodRuntime {
        workspace_id: "workspace-1".into(),
        datacenter_id: "dc-1".into(),
        gpu_id: "gpu-1".into(),
        volume_size_gb: 19,
    }).await.unwrap();

    assert_eq!(runtime.state, RunpodRuntimeState::Ready);
    assert_eq!(fakes.provider.calls(), vec![
        "create_network_volume", "start_provisioner_pod", "wait_for_provisioner",
        "terminate_provisioner_pod", "create_template", "create_endpoint",
    ]);
    assert_eq!(fakes.repository.running_steps(), vec![
        RunpodProvisionStep::CreateNetworkVolume,
        RunpodProvisionStep::StartProvisionerPod,
        RunpodProvisionStep::PollProvisioner,
        RunpodProvisionStep::TerminateProvisionerPod,
        RunpodProvisionStep::CreateTemplate,
        RunpodProvisionStep::CreateEndpoint,
    ]);
    assert_eq!(fakes.repository.last_operation_state(), LifecycleOperationState::Succeeded);
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::provision_persists_each_current_step_before_the_provider_call -- --exact
```

Expected: compilation fails because `RunpodRuntimeService` and `ProvisionRunpodRuntime` do not exist.

- [ ] **Step 3: Implement service dependencies and preflight**

```rust
pub struct ProvisionRunpodRuntime {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
}

pub struct RunpodRuntimeService<'a> {
    pub workspaces: &'a dyn WorkspaceRepository,
    pub workflows: &'a dyn WorkflowCatalog,
    pub runtimes: &'a dyn RunpodRuntimeRepository,
    pub runtime_catalog: &'a dyn RunpodRuntimeCatalog,
    pub lifecycle: &'a dyn LifecycleOperationRepository,
    pub secrets: &'a dyn SecretStore,
    pub provider: &'a dyn RunpodRuntimeProvider,
}
```

Preflight order is exact: load workspace; when `attached_runtime == Some(RuntimeKind::Runpod)`, load that runtime and use `begin_provision` on a clone to return `AlreadyProvisioned`, `RuntimeFailed`, or `OperationInProgress` from its actual state; reject `has_running`; load workflow; select RunPod requirements; resolve runtime definition; load RunPod API key; load Hugging Face API key only when summary requires it. Return `CredentialMissing` before creating runtime/operation if a required key is absent.

- [ ] **Step 4: Implement the six-step transition algorithm**

Use one helper that persists the current step before each provider call:

```rust
async fn set_provision_step(
    &self,
    runtime: &RunpodRuntime,
    operation: &mut LifecycleOperation,
    step: RunpodProvisionStep,
) -> Result<(), RunpodRuntimeError> {
    operation.set_provision_step(step, OffsetDateTime::now_utc())?;
    self.runtimes.save_transition(runtime, operation).await.map_err(Into::into)
}
```

Provision sequence:

```rust
let mut runtime = RunpodRuntime::new_provisioning(command.workspace_id.clone(), config);
let mut operation = LifecycleOperation::runpod_provision(
    Uuid::new_v4().to_string(),
    command.workspace_id.clone(),
    Uuid::new_v4().to_string(),
    RunpodProvisionStep::CreateNetworkVolume,
    OffsetDateTime::now_utc(),
);
self.runtimes.save_transition(&runtime, &operation).await?;

let volume_id = self.provider.create_network_volume(&runpod_key, CreateNetworkVolume {
    workspace_id: command.workspace_id.clone(),
    datacenter_id: command.datacenter_id.clone(),
    size_gb: command.volume_size_gb,
}).await.map_err(RunpodRuntimeError::from)?;
runtime.resources.network_volume_id = Some(volume_id.clone());
self.set_provision_step(&runtime, &mut operation, RunpodProvisionStep::StartProvisionerPod).await?;

let pod_id = self.provider.start_provisioner_pod(&runpod_key, StartProvisionerPod {
    workspace_id: command.workspace_id.clone(),
    datacenter_id: command.datacenter_id.clone(),
    network_volume_id: volume_id.clone(),
    provisioner_image_ref: definition.provisioner_contract.image_ref.clone(),
    required_model_assets: workflow.model_assets.clone(),
    hugging_face_api_key,
}).await.map_err(RunpodRuntimeError::from)?;
runtime.resources.provisioner_pod_id = Some(pod_id.clone());
self.set_provision_step(&runtime, &mut operation, RunpodProvisionStep::PollProvisioner).await?;
self.provider.wait_for_provisioner(&runpod_key, &command.workspace_id, &pod_id).await.map_err(RunpodRuntimeError::from)?;

self.set_provision_step(&runtime, &mut operation, RunpodProvisionStep::TerminateProvisionerPod).await?;
self.provider.terminate_provisioner_pod(&runpod_key, &pod_id).await.map_err(RunpodRuntimeError::from)?;
runtime.resources.provisioner_pod_id = None;
self.set_provision_step(&runtime, &mut operation, RunpodProvisionStep::CreateTemplate).await?;

let template_id = self.provider.create_template(&runpod_key, CreateTemplate {
    workspace_id: command.workspace_id.clone(),
    image_ref: definition.endpoint_contract.image_ref.clone(),
}).await.map_err(RunpodRuntimeError::from)?;
runtime.resources.template_id = Some(template_id.clone());
self.set_provision_step(&runtime, &mut operation, RunpodProvisionStep::CreateEndpoint).await?;

let endpoint_id = self.provider.create_endpoint(&runpod_key, CreateEndpoint {
    workspace_id: command.workspace_id.clone(),
    datacenter_id: command.datacenter_id.clone(),
    gpu_id: command.gpu_id.clone(),
    network_volume_id: volume_id,
    template_id,
}).await.map_err(RunpodRuntimeError::from)?;
runtime.resources.endpoint_id = Some(endpoint_id);

runtime.mark_ready()?;
operation.succeed(OffsetDateTime::now_utc())?;
self.runtimes.save_transition(&runtime, &operation).await?;
```

Each successful resource call updates `runtime.resources` before `set_provision_step` persists the next step. After provisioner termination, set `provisioner_pod_id = None`. `StartProvisionerPod` receives workflow model assets, resolved provisioner image, and optional Hugging Face credential. `CreateTemplate` receives the resolved endpoint image.

- [ ] **Step 5: Add and satisfy failure behavior**

Add a table-driven test where each provider call fails once. Assert: provider calls stop at that method; runtime becomes `Failed`; operation becomes `Failed`; current progress is the failing step; resource IDs from earlier successful calls remain.

Replace every happy-path `.await.map_err(RunpodRuntimeError::from)?` provider expression from Step 4 with this exact failure shape before considering the task complete:

```rust
let value = match provider_call.await {
    Ok(value) => value,
    Err(error) => {
        self.fail_transition(&mut runtime, &mut operation).await?;
        return Err(RunpodRuntimeError::from(error));
    }
};
```

For provider methods returning `()`, omit the `let value =` binding and use the same `match` branches.

Implement one failure helper:

```rust
async fn fail_transition(
    &self,
    runtime: &mut RunpodRuntime,
    operation: &mut LifecycleOperation,
) -> Result<(), RunpodRuntimeError> {
    runtime.mark_failed();
    operation.fail(OffsetDateTime::now_utc())?;
    self.runtimes.save_transition(runtime, operation).await.map_err(Into::into)
}
```

Call it for every provider failure after the initial transition has been saved. Return only the typed provider/application error after the failed state is durable.

- [ ] **Step 6: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: happy-path order and all provider-failure cases pass without sleeps or network access.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/application/runtimes/runpod
git commit -m "feat(runpod): add provision workflow"
```

---

### Task 7: RunPod Cleanup and Interrupted-Operation Recovery

**Files:**
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`

**Interfaces:**
- Consumes: Task 6 service and fakes.
- Produces: `cleanup(workspace_id)` and `fail_interrupted()` behaviors.

- [ ] **Step 1: Write failing cleanup tests**

```rust
#[tokio::test]
async fn cleanup_runs_every_step_and_removes_the_runtime() {
    let fakes = CleanupFakes::ready_runtime();

    fakes.service().cleanup("workspace-1").await.unwrap();

    assert_eq!(fakes.provider.calls(), vec![
        "delete_endpoint", "delete_template", "terminate_provisioner_pod", "delete_network_volume",
    ]);
    assert_eq!(fakes.repository.running_cleanup_steps(), vec![
        RunpodCleanupStep::DeleteEndpoint,
        RunpodCleanupStep::DeleteTemplate,
        RunpodCleanupStep::TerminateProvisionerPod,
        RunpodCleanupStep::DeleteNetworkVolume,
    ]);
    assert!(fakes.repository.runtime_was_removed());
}

#[tokio::test]
async fn cleanup_skips_absent_resource_ids_but_still_records_each_step() {
    let fakes = CleanupFakes::failed_partial_runtime();

    fakes.service().cleanup("workspace-1").await.unwrap();

    assert_eq!(fakes.repository.running_cleanup_steps().len(), 4);
    assert_eq!(fakes.provider.calls(), vec!["delete_network_volume"]);
}

#[tokio::test]
async fn cleanup_without_runtime_is_explicit_not_provisioned() {
    assert_eq!(CleanupFakes::without_runtime().service().cleanup("workspace-1").await, Err(RunpodRuntimeError::NotProvisioned));
}
```

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests::cleanup_runs_every_step_and_removes_the_runtime -- --exact
```

Expected: compilation fails because `cleanup` does not exist.

- [ ] **Step 3: Implement cleanup**

Load the runtime, call `begin_cleanup`, create a `Running` cleanup operation at `DeleteEndpoint`, and atomically save it. For each approved step: persist the current step first; call the provider only when the corresponding ID exists; clear the ID after success; then persist the next step. Adapter-level NotFound is already represented as success by the provider port.

Finish with:

```rust
operation.succeed(OffsetDateTime::now_utc())?;
self.runtimes.save_transition(&runtime, &operation).await?;
```

The repository recognizes `CleaningUp + Succeeded` and deletes the provider extension and runtime anchor while retaining the journal row.

On any provider error, keep earlier cleared IDs, mark runtime/operation Failed through `fail_transition`, and retain the failing cleanup step.

- [ ] **Step 4: Write failing startup recovery test**

```rust
#[tokio::test]
async fn startup_marks_running_operations_and_runtimes_failed() {
    let fakes = RecoveryFakes::with_running_provision_and_cleanup();

    fakes.service().fail_interrupted().await.unwrap();

    assert_eq!(fakes.repository.saved_states(), vec![
        (RunpodRuntimeState::Failed, LifecycleOperationState::Failed),
        (RunpodRuntimeState::Failed, LifecycleOperationState::Failed),
    ]);
    assert_eq!(fakes.repository.saved_trace_ids(), vec!["trace-provision", "trace-cleanup"]);
}
```

- [ ] **Step 5: Implement startup recovery**

`fail_interrupted` calls `LifecycleOperationRepository::running`, matches `LifecycleProgress::Runpod`, loads the corresponding runtime, leaves progress/trace unchanged, marks both models Failed, and saves each pair through `save_transition`. Do not call provider APIs and do not attempt reconciliation.

- [ ] **Step 6: Run GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests -- --nocapture
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: cleanup, partial cleanup, missing runtime, failure retention, and interrupted recovery tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/application/runtimes/runpod/service.rs src-tauri/src/application/runtimes/runpod/test_support.rs
git commit -m "feat(runpod): add cleanup and recovery"
```

---

### Task 8: Bundled, Keyring, and Identity Adapters

**Files:**
- Create: `src-tauri/src/adapters/mod.rs`
- Create: `src-tauri/src/adapters/bundled/{mod.rs,workflow_catalog.rs,runpod_runtime_catalog.rs}`
- Create: `src-tauri/src/adapters/keyring/{mod.rs,secret_store.rs}`
- Create: `src-tauri/src/adapters/runpod/{mod.rs,identity_provider.rs}`
- Create: `src-tauri/src/adapters/hugging_face/{mod.rs,identity_provider.rs}`
- Modify: `src-tauri/src/infra/keyring/storage.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: application ports from Tasks 1, 4, and 5 plus existing `Catalog`, `KeyringStorage`, `RunpodClient`, and `HuggingFaceClient`.
- Produces: concrete adapters with no application policy beyond mapping and port semantics.

- [ ] **Step 1: Create adapter module exports**

```rust
// src-tauri/src/adapters/mod.rs
pub mod bundled;
pub mod hugging_face;
pub mod keyring;
pub mod runpod;
```

Add `pub mod adapters;` to `src-tauri/src/lib.rs`. Create each nested `mod.rs` exporting only its concrete adapter types.

- [ ] **Step 2: Implement `BundledCatalogAdapter`**

Use one struct in `adapters/bundled/mod.rs`:

```rust
#[derive(Debug)]
pub struct BundledCatalogAdapter { catalog: crate::infra::bundled::Catalog }
```

Add `pub fn new(catalog: Catalog) -> Self`; do not let the adapter discover filesystem paths.

Implement `WorkflowCatalog` by calling `entries::workflows::Entry::all/get`. Map generated `name` and `description` newtypes with `String::from(value)`, map `required_volume_size_gb` with `NonZeroU64::get`, map `model_assets.model_assets` with `serde_json::to_value` so the application value is the worker-expected JSON array, and map the execution contract and graph with `serde_json::to_value`. Map generated RunPod requirements into typed `RuntimeContractRequirements::Runpod`. Sort summaries by `(&id, &revision)`.

Before dropping generated reference contract paths, require `runtime_preset_ref.contract == "catalog/contracts/runtime_preset_revision"` and both RunPod contract refs to equal `"catalog/contracts/runtime_contract_revision"`; any mismatch is `InvalidCatalog`.

Implement `RunpodRuntimeCatalog::resolve` with three exact lookups:

```rust
let preset = runtime_presets::Entry::get(&self.catalog, (&preset.id, &preset.revision)).await?;
let provisioner = runtime_contracts::Entry::get(&self.catalog, (&requirements.provisioner_contract_ref.id, &requirements.provisioner_contract_ref.revision)).await?;
let endpoint = runtime_contracts::Entry::get(&self.catalog, (&requirements.endpoint_contract_ref.id, &requirements.endpoint_contract_ref.revision)).await?;
```

Any missing internal reference or serialization failure maps to `InvalidCatalog`; I/O maps to `Unavailable`. Do not return raw generated models.

- [ ] **Step 3: Implement keyring account ownership and strict semantics**

```rust
const RUNPOD_ACCOUNT: &str = "runpod-api-key";
const HUGGING_FACE_ACCOUNT: &str = "hugging-face-api-key";

fn account(kind: SecretKind) -> &'static str {
    match kind {
        SecretKind::RunpodApiKey => RUNPOD_ACCOUNT,
        SecretKind::HuggingFaceApiKey => HUGGING_FACE_ACCOUNT,
    }
}
```

Add `KeyringStorage::exists(account)` to the raw primitive. It runs `Entry::get_password` inside the existing blocking helper but returns only a boolean and never returns the password to the adapter. `KeyringSecretStore::new(storage: KeyringStorage)` stores the injected raw primitive; `exists` calls the raw boolean method. `insert` checks `exists` before raw `set` and returns `AlreadyExists`. `delete` checks `exists`, returns `NotFound` when absent, then calls raw `delete`. Do not add account names to application or infra.

- [ ] **Step 4: Implement the two common identity adapters**

`RunpodIdentityAdapter::new(client: RunpodClient)` stores the injected client and calls `RunpodClient::myself`. The generated GraphQL shape is `MyselfResponse { myself: Option<myself::MyselfMyself> }`; map it exactly as:

```rust
let myself = response.myself.ok_or(SecretIdentityProviderError::Unavailable)?;
Ok(Identity { key_name: None, username: None, email: myself.email })
```

`HuggingFaceIdentityAdapter` maps:

```rust
Identity {
    key_name: response.auth.access_token.map(|token| token.display_name),
    username: Some(response.name),
    email: response.email,
}
```

Construct it with `HuggingFaceIdentityAdapter::new(client: HuggingFaceClient)`.

Map `NetworkError::Unauthorized` to `InvalidCredential`; all other network errors to `Unavailable`. Do not persist Identity.

- [ ] **Step 5: Compile adapters without adding tests**

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: adapters compile; no new `#[cfg(test)]` appears under `src-tauri/src/adapters` or `src-tauri/src/infra`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/infra/keyring/storage.rs src-tauri/src/adapters
git commit -m "feat(adapters): add catalogs and secrets"
```

---

### Task 9: RunPod Provider Adapter and Raw Provisioner Status Primitive

**Files:**
- Modify: `src-tauri/src/infra/clients/errors.rs`
- Modify: `src-tauri/src/infra/clients/http.rs`
- Modify: `src-tauri/src/infra/clients/runpod/client.rs`
- Modify: `src-tauri/src/infra/clients/runpod/mod.rs`
- Create: `src-tauri/src/adapters/runpod/runtime_provider.rs`
- Modify: `src-tauri/src/adapters/runpod/mod.rs`

**Interfaces:**
- Consumes: existing raw RunPod REST DTOs/client and Task 5 `RunpodRuntimeProvider`.
- Produces: cleanup-safe NotFound mapping and concrete provider operations, including provisioner polling.

The concrete type is `RunpodRuntimeProviderAdapter::new(client: RunpodClient)`; it stores no credentials.

- [ ] **Step 1: Preserve raw HTTP NotFound and add provisioner status response**

Add `NetworkError::NotFound` and map `StatusCode::NOT_FOUND` to it in `infra/clients/http.rs`. Do not change other status semantics.

Add raw types beside `RunpodClient`:

```rust
#[derive(serde::Deserialize)]
pub struct ProvisionerStatusResponse {
    pub status: String,
    pub error: Option<ProvisionerFailure>,
}

#[derive(serde::Deserialize)]
pub struct ProvisionerFailure { pub code: String, pub message: String }
```

Add `RunpodClient::provisioner_status(&SecretString, pod_id)` using URL `https://{pod_id}-8000.proxy.runpod.net/status`, bearer auth, and existing `ResponseExt::into_json`. This is one raw request; polling policy belongs in the adapter.

- [ ] **Step 2: Implement generated DTO mappings**

Use constants in `adapters/runpod/runtime_provider.rs`:

```rust
const RESOURCE_PREFIX: &str = "luma-forge";
const PROVISIONER_PORT: &str = "8000/http";
const ENDPOINT_WORKERS_MIN: i64 = 0;
const ENDPOINT_WORKERS_MAX: i64 = 1;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const POLL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
```

Map application commands to existing generated inputs:

- network volume: required `data_center_id`, `name = luma-forge-{workspace_id}-volume`, `size`;
- provisioner pod: CPU compute, selected datacenter, provisioner image, network volume, `8000/http`, and env for bearer token/model assets/optional Hugging Face key;
- template: endpoint image, `is_serverless = Some(true)`, private, deterministic name;
- endpoint: selected datacenter/GPU, network volume/template IDs, workers min/max, deterministic name.

Extract `id` from optional generated response fields; missing IDs map to `Unavailable`. Use struct defaults for generated optional fields; do not expose generated types through the port.

- [ ] **Step 3: Implement credential-safe bearer derivation and polling**

Derive the provisioner bearer token inside the adapter from the RunPod `SecretString` and workspace ID using existing `hmac`/`sha2`; expose only the derived hex string to the request env and status call. Never log either value.

`wait_for_provisioner` loops until `Succeeded`, maps provider `Failed` to `ProvisionerFailed`, and maps timeout/invalid response to `Unavailable`. Sleep with `tokio::time::sleep(POLL_INTERVAL)`. This polling is adapter behavior and receives no application tests.

- [ ] **Step 4: Make cleanup calls idempotent at the adapter boundary**

For `delete_endpoint`, `delete_template`, `terminate_provisioner_pod`, and `delete_network_volume`, map both `Ok(())` and `Err(NetworkError::NotFound)` to `Ok(())`. Map unauthorized to `Unauthorized`, all other errors to `Unavailable`.

- [ ] **Step 5: Compile without Tauri/frontend verification**

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: native crate compiles. Do not run Tauri codegen, `bun run build`, or frontend lint.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/infra/clients/errors.rs src-tauri/src/infra/clients/http.rs src-tauri/src/infra/clients/runpod src-tauri/src/adapters/runpod
git commit -m "feat(runpod): adapt runtime provider"
```

---

### Task 10: SQLite Entities and Repository Adapters

**Files:**
- Modify: `src-tauri/src/infra/sqlite/entities/mod.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/workspaces.rs`
- Create: `src-tauri/src/infra/sqlite/entities/workspace_runtimes.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/runpod_workspace_runtimes.rs`
- Modify: `src-tauri/src/infra/sqlite/entities/lifecycle_operations.rs`
- Delete: `src-tauri/src/infra/sqlite/entities/runpod_operation_payloads.rs`
- Create: `src-tauri/src/infra/sqlite/entities/runpod_lifecycle_progress.rs`
- Create: `src-tauri/src/adapters/sqlite/mod.rs`
- Create: `src-tauri/src/adapters/sqlite/workspace_repository.rs`
- Create: `src-tauri/src/adapters/sqlite/lifecycle_operation_repository.rs`
- Create: `src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs`
- Modify: `src-tauri/src/adapters/mod.rs`

**Interfaces:**
- Consumes: existing `SqliteInfraDatabase::connection`, SeaORM dense entities, and all repository ports.
- Produces: current schema and concrete repository adapters; no adapter tests.

Add `pub mod sqlite;` to `src-tauri/src/adapters/mod.rs` only in this task, when the module exists.

- [ ] **Step 1: Change entity shape to the approved current contract**

`workspaces` fields become only `id`, `workflow_id`, `workflow_revision`, and `created_at`. Remove workspace state/runtime kind/update timestamp and the cascade lifecycle relation.

Create `workspace_runtimes`:

```rust
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspace_runtimes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workspace_id: String,
    pub provider_kind: String,
    #[sea_orm(belongs_to, from = "workspace_id", to = "id", on_delete = "Cascade")]
    pub workspace: HasOne<super::workspaces::Entity>,
}
```

`runpod_workspace_runtimes` belongs to `workspace_runtimes`, adds `state`, and retains immutable config/resource IDs.

`lifecycle_operations` adds `trace_id` and nullable unique `running_workspace_id`. Set `running_workspace_id = Some(workspace_id)` only for Running rows and `None` for terminal rows; this enforces one Running operation while allowing unlimited history. Keep historical `workspace_id` without a foreign key or cascade.

Declare the active-operation column exactly as:

```rust
#[sea_orm(unique)]
pub running_workspace_id: Option<String>,
```

Rename the payload entity/table to `runpod_lifecycle_progress` with `operation_id` and `step`; keep cascade only from progress to lifecycle operation.

- [ ] **Step 2: Implement workspace repository mapping**

`SqliteWorkspaceRepository` wraps `DatabaseConnection`. `create` inserts only the workspace row. `get/list` left-join `workspace_runtimes` and map `provider_kind = "runpod"` to `RuntimeKind::Runpod`; unknown kinds return `CorruptData`. `delete` returns whether one row was deleted. Do not query bundled catalogs.

Each SQLite adapter owns a cloned `DatabaseConnection` and has `pub fn new(connection: DatabaseConnection) -> Self`.

- [ ] **Step 3: Implement lifecycle journal reads**

Map operation state/kind/progress strings with explicit `match`; unknown values return `CorruptData`. `recent` and `recent_for_workspace` order `created_at DESC` and apply the requested limit. `running`/`has_running` filter `running_workspace_id IS NOT NULL`. Load RunPod progress by operation IDs without exposing SeaORM models.

- [ ] **Step 4: Implement atomic RunPod transition persistence**

`SqliteRunpodRuntimeRepository::save_transition` opens one SeaORM transaction and performs these cases:

```text
Provisioning + Running first step:
  insert workspace_runtimes anchor(kind=runpod)
  insert runpod_workspace_runtimes
  insert lifecycle_operations(running_workspace_id=workspace_id)
  insert runpod_lifecycle_progress

Running update:
  update runpod runtime state/resources
  update lifecycle timestamps/state/running_workspace_id
  update progress step

CleaningUp + Succeeded:
  delete runpod runtime extension
  delete workspace runtime anchor
  update lifecycle row to Succeeded with running_workspace_id=NULL
  retain lifecycle/progress rows

Any Failed transition:
  update runtime state to Failed
  update lifecycle row to Failed with running_workspace_id=NULL
  retain current progress
```

`get` joins anchor and RunPod extension; missing extension for a RunPod anchor is `CorruptData`. Map unique active-operation/anchor conflicts to typed repository errors only when the operation being attempted is known; otherwise return `Unavailable`. Do not parse raw SQLite error strings.

- [ ] **Step 5: Compile schema sync and adapters**

```bash
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: SeaORM derives, entity registry, and all repository implementations compile. Do not add SQLite tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/infra/sqlite/entities src-tauri/src/adapters/mod.rs src-tauri/src/adapters/sqlite
git commit -m "feat(sqlite): adapt application repositories"
```

---

### Task 11: Full Application Verification and Scope Audit

**Files:**
- No changes expected. A failure returns execution to the task that owns the failing file; this task does not accumulate unrelated cleanup.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: a compiling native crate with all application behavioral tests passing and no forbidden layer dependencies.

- [ ] **Step 1: Run the complete application test suite**

```bash
cargo test --manifest-path src-tauri/Cargo.toml application:: -- --nocapture
```

Expected: all application behavioral tests pass; no adapter/infra live calls occur.

- [ ] **Step 2: Run native verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: tests and formatting pass. Clippy exits 0; if it fails only on a pre-existing warning outside files touched by this plan, record the exact file/line and do not refactor adjacent code.

- [ ] **Step 3: Verify dependency direction mechanically**

```bash
rg -n "crate::(infra|adapters)|tauri|specta|sea_orm|reqwest|keyring" src-tauri/src/application
rg -n "crate::(application|adapters)" src-tauri/src/infra
rg -n "ExposeSecret" src-tauri/src/application
rg -n "SecretString" src-tauri/src/application
rg -n "ExposeSecret" src-tauri/src/adapters
```

Expected:

- first command returns no forbidden imports from application;
- second command returns no matches;
- third command returns no matches; the fourth shows `SecretString` only in trusted secrets/runtime services and their ports; the fifth shows raw exposure only inside provider/keyring request construction and never in models, errors, logs, or tests.

- [ ] **Step 4: Confirm explicitly skipped surfaces**

Do not run or repair any of:

```text
bun run codegen:commands
bun run build
bun run lint
frontend runtime
```

Confirm no files under `src/` or `src/generated/commands.ts` changed.
