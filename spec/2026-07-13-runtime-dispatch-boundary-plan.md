# Runtime Dispatch Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move runtime lifecycle dispatch from the Tauri facade into a provider-neutral application service without changing observable behavior, persistence, or frontend contracts.

**Architecture:** Introduce one closed `application::runtimes::RuntimeService` that owns provision, cleanup, and recovery routing across the concrete supported runtime services. Keep RunPod lifecycle implementation provider-specific, keep SQLite's closed persistence dispatch unchanged, and reduce the facade to DTO/error mapping plus direct RunPod placement lookup.

**Tech Stack:** Rust, Tokio, async-trait ports, Tauri, Specta, SQLite adapters, existing diagnostic macros, Cargo tests.

## Global Constraints

- No new runtime is added.
- No trait registry, factory, plugin loader, common `RuntimeService` trait, or dynamic runtime registration is introduced.
- No new dependency is added.
- SQLite tables, persisted values, transition repositories, and provider-specific persistence dispatch remain unchanged.
- Tauri command names, DTO shapes, generated TypeScript bindings, and frontend behavior remain unchanged.
- Existing facade error codes and their mappings remain unchanged.
- Existing `commit -> emit events -> continue detached provider work` behavior remains unchanged.
- Provider adapters continue implementing one provider port each and never dispatch between runtimes.
- Shared runtime unions remain closed and compiler-checked.
- Follow the pre-v1 policy: update all callers directly; add no compatibility alias or fallback for `RunpodRuntimeError` or `RuntimeDispatcher`.
- Keep secrets out of DTOs, diagnostics, errors, fixtures, and persisted workspace snapshots.
- Use Conventional Commits for every implementation commit.
- Final native verification is exactly:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

---

## File Structure

- Create `src-tauri/src/application/runtimes/service.rs`: provider-neutral lifecycle command and closed application dispatch.
- Modify `src-tauri/src/application/runtimes/errors.rs`: common lifecycle error and shared error conversions.
- Modify `src-tauri/src/application/runtimes/mod.rs`: expose `RuntimeError`, `ProvisionRuntime`, and `RuntimeService`.
- Modify `src-tauri/src/application/runtimes/model.rs`: own RunPod narrowing on the shared closed enums.
- Modify `src-tauri/src/application/runtimes/runpod/errors.rs`: retain only RunPod provider/catalog conversions into the common error.
- Modify `src-tauri/src/application/runtimes/runpod/mod.rs`: stop exporting a RunPod-specific lifecycle error.
- Modify `src-tauri/src/application/runtimes/runpod/service.rs`: return `RuntimeError`, use shared narrowing, and scan all workflow requirements.
- Modify `src-tauri/src/application/runtimes/runpod/test_support.rs`: expose the existing fakes to application dispatcher tests without adding a second fixture stack.
- Modify `src-tauri/src/facade/errors.rs`: map `RuntimeError` to the existing command-specific codes.
- Modify `src-tauri/src/facade/state.rs`: delete facade lifecycle dispatch and map transport input into `ProvisionRuntime`.
- Modify `src-tauri/src/lib.rs`: construct the RunPod service once, clone it into application dispatch and placement lookup, and share repository `Arc`s.
- Do not modify `src-tauri/src/adapters/**`, `src-tauri/src/infra/**`, database migrations, `src/generated/commands.ts`, or frontend code.

### Task 1: Promote the lifecycle error to the common runtime boundary

**Files:**

- Modify: `src-tauri/src/application/runtimes/errors.rs:1-5`
- Modify: `src-tauri/src/application/runtimes/mod.rs:1-17`
- Modify: `src-tauri/src/application/runtimes/runpod/errors.rs:1-104`
- Modify: `src-tauri/src/application/runtimes/runpod/mod.rs:1-19`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs:6-1018`
- Modify: `src-tauri/src/facade/errors.rs:1-389`
- Modify: `src-tauri/src/facade/state.rs:1-337`

**Interfaces:**

- Consumes: existing `RuntimePersistenceError`, `RuntimeOperationRepositoryError`, `SecretStoreError`, `RuntimeOperationError`, `RunpodRuntimeProviderError`, and `RunpodRuntimeCatalogError`.
- Produces: `application::runtimes::RuntimeError`, used by every lifecycle service and facade command mapping in later tasks.

- [ ] **Step 1: Add failing common error-mapping tests**

Append this test module to `src-tauri/src/application/runtimes/errors.rs` before defining `RuntimeError`:

```rust
#[cfg(test)]
mod tests {
    use crate::application::{
        runtimes::ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
        secrets::SecretStoreError,
    };

    use super::{RuntimeError, RuntimeOperationError};

    #[test]
    fn shared_persistence_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::AlreadyExists),
            RuntimeError::AlreadyProvisioned
        );
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::OperationAlreadyRunning),
            RuntimeError::OperationInProgress
        );
        for error in [
            RuntimePersistenceError::NotFound,
            RuntimePersistenceError::Unavailable,
            RuntimePersistenceError::CorruptData,
        ] {
            assert_eq!(
                RuntimeError::from(error),
                RuntimeError::PersistenceUnavailable
            );
        }
    }

    #[test]
    fn shared_operation_secret_and_transition_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimeOperationRepositoryError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(SecretStoreError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(RuntimeOperationError::InvalidTransition),
            RuntimeError::InvalidTransition
        );
    }
}
```

- [ ] **Step 2: Run the focused test and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::errors::tests
```

Expected: compilation fails because `RuntimeError` is not defined in `application::runtimes::errors`.

- [ ] **Step 3: Define the common error and shared conversions**

Replace `src-tauri/src/application/runtimes/errors.rs` with:

```rust
use crate::application::secrets::SecretStoreError;

use super::ports::{RuntimeOperationRepositoryError, RuntimePersistenceError};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeOperationError {
    #[error("runtime operation transition is invalid")]
    InvalidTransition,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
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
    #[error("runtime provider rejected the credential")]
    InvalidCredential,
    #[error("runtime provider is unavailable")]
    ProviderUnavailable,
    #[error("application catalog is unavailable or invalid")]
    CatalogUnavailable,
    #[error("runtime persistence is unavailable or invalid")]
    PersistenceUnavailable,
    #[error("runtime transition is invalid")]
    InvalidTransition,
}

impl From<RuntimePersistenceError> for RuntimeError {
    fn from(error: RuntimePersistenceError) -> Self {
        match error {
            RuntimePersistenceError::AlreadyExists => Self::AlreadyProvisioned,
            RuntimePersistenceError::OperationAlreadyRunning => Self::OperationInProgress,
            RuntimePersistenceError::NotFound
            | RuntimePersistenceError::Unavailable
            | RuntimePersistenceError::CorruptData => Self::PersistenceUnavailable,
        }
    }
}

impl From<RuntimeOperationRepositoryError> for RuntimeError {
    fn from(_: RuntimeOperationRepositoryError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<SecretStoreError> for RuntimeError {
    fn from(_: SecretStoreError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<RuntimeOperationError> for RuntimeError {
    fn from(_: RuntimeOperationError) -> Self {
        Self::InvalidTransition
    }
}

#[cfg(test)]
mod tests {
    use crate::application::{
        runtimes::ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
        secrets::SecretStoreError,
    };

    use super::{RuntimeError, RuntimeOperationError};

    #[test]
    fn shared_persistence_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::AlreadyExists),
            RuntimeError::AlreadyProvisioned
        );
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::OperationAlreadyRunning),
            RuntimeError::OperationInProgress
        );
        for error in [
            RuntimePersistenceError::NotFound,
            RuntimePersistenceError::Unavailable,
            RuntimePersistenceError::CorruptData,
        ] {
            assert_eq!(
                RuntimeError::from(error),
                RuntimeError::PersistenceUnavailable
            );
        }
    }

    #[test]
    fn shared_operation_secret_and_transition_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimeOperationRepositoryError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(SecretStoreError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(RuntimeOperationError::InvalidTransition),
            RuntimeError::InvalidTransition
        );
    }
}
```

In `src-tauri/src/application/runtimes/mod.rs`, export both errors:

```rust
pub use errors::{RuntimeError, RuntimeOperationError};
```

- [ ] **Step 4: Keep only provider-specific conversions in the RunPod error module**

Replace `src-tauri/src/application/runtimes/runpod/errors.rs` with:

```rust
use crate::application::runtimes::RuntimeError;

use super::ports::{RunpodRuntimeCatalogError, RunpodRuntimeProviderError};

impl From<RunpodRuntimeProviderError> for RuntimeError {
    fn from(error: RunpodRuntimeProviderError) -> Self {
        match error {
            RunpodRuntimeProviderError::Unauthorized => Self::InvalidCredential,
            RunpodRuntimeProviderError::Unavailable
            | RunpodRuntimeProviderError::ProvisionerFailed => Self::ProviderUnavailable,
        }
    }
}

impl From<RunpodRuntimeCatalogError> for RuntimeError {
    fn from(_: RunpodRuntimeCatalogError) -> Self {
        Self::CatalogUnavailable
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::RuntimeError;

    use super::RunpodRuntimeProviderError;

    #[test]
    fn provider_errors_preserve_invalid_credentials() {
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::Unauthorized),
            RuntimeError::InvalidCredential
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::Unavailable),
            RuntimeError::ProviderUnavailable
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::ProvisionerFailed),
            RuntimeError::ProviderUnavailable
        );
    }
}
```

Delete this export from `src-tauri/src/application/runtimes/runpod/mod.rs`:

```rust
pub use errors::RunpodRuntimeError;
```

Keep `mod errors;` so the provider-specific `From` implementations remain compiled.

- [ ] **Step 5: Replace every lifecycle reference with the common error type**

Apply these exact token-level edits; do not change any function body or error mapping in this step:

| File | Exact edit |
| --- | --- |
| `src-tauri/src/application/runtimes/runpod/service.rs` | Add `RuntimeError` to the production `crate::application::runtimes` import, remove `RunpodRuntimeError` from the production `super` import, replace every lifecycle type and variant path with `RuntimeError`, remove `RunpodRuntimeError` from the test module's `runtimes::runpod` import, and add `RuntimeError` to the test module's parent `runtimes` import. |
| `src-tauri/src/facade/state.rs` | Import `RuntimeError` from `application::runtimes`, remove `RunpodRuntimeError` from the RunPod import, and replace every `RunpodRuntimeError` token with `RuntimeError`; retain `RuntimeDispatcher` until Task 4. |
| `src-tauri/src/facade/errors.rs` | Import `RuntimeError` from `application::runtimes`, remove the RunPod error import, and replace every `RunpodRuntimeError` token with `RuntimeError`. |

The resulting application import in `src-tauri/src/facade/errors.rs` is:

```rust
use crate::application::{
    runtimes::{ports::RuntimeOperationRepositoryError, RuntimeError},
    secrets::SecretsError,
    workspace::WorkspaceError,
};
```

The three complete command mappings are:

```rust
impl From<RuntimeError> for CommandError<ProvisionWorkspaceErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::WorkspaceNotFound => ProvisionWorkspaceErrorCode::WorkspaceNotFound,
            RuntimeError::WorkflowNotFound => ProvisionWorkspaceErrorCode::WorkflowNotFound,
            RuntimeError::AlreadyProvisioned => {
                ProvisionWorkspaceErrorCode::AlreadyProvisioned
            }
            RuntimeError::RuntimeFailed => ProvisionWorkspaceErrorCode::RuntimeFailed,
            RuntimeError::OperationInProgress => {
                ProvisionWorkspaceErrorCode::OperationInProgress
            }
            RuntimeError::CredentialMissing => ProvisionWorkspaceErrorCode::CredentialMissing,
            RuntimeError::CatalogUnavailable => ProvisionWorkspaceErrorCode::CatalogUnavailable,
            RuntimeError::PersistenceUnavailable => {
                ProvisionWorkspaceErrorCode::PersistenceUnavailable
            }
            RuntimeError::InvalidTransition => ProvisionWorkspaceErrorCode::InvalidTransition,
            RuntimeError::NotProvisioned
            | RuntimeError::InvalidCredential
            | RuntimeError::ProviderUnavailable => ProvisionWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<RuntimeError> for CommandError<CleanupWorkspaceErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::WorkspaceNotFound => CleanupWorkspaceErrorCode::WorkspaceNotFound,
            RuntimeError::NotProvisioned => CleanupWorkspaceErrorCode::NotProvisioned,
            RuntimeError::OperationInProgress => {
                CleanupWorkspaceErrorCode::OperationInProgress
            }
            RuntimeError::CredentialMissing => CleanupWorkspaceErrorCode::CredentialMissing,
            RuntimeError::PersistenceUnavailable => {
                CleanupWorkspaceErrorCode::PersistenceUnavailable
            }
            RuntimeError::InvalidTransition => CleanupWorkspaceErrorCode::InvalidTransition,
            RuntimeError::WorkflowNotFound
            | RuntimeError::AlreadyProvisioned
            | RuntimeError::RuntimeFailed
            | RuntimeError::InvalidCredential
            | RuntimeError::ProviderUnavailable
            | RuntimeError::CatalogUnavailable => CleanupWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<RuntimeError> for CommandError<GetRunpodPlacementErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::CredentialMissing => GetRunpodPlacementErrorCode::CredentialMissing,
            RuntimeError::InvalidCredential => GetRunpodPlacementErrorCode::InvalidCredential,
            RuntimeError::ProviderUnavailable => {
                GetRunpodPlacementErrorCode::ProviderUnavailable
            }
            RuntimeError::WorkspaceNotFound
            | RuntimeError::WorkflowNotFound
            | RuntimeError::AlreadyProvisioned
            | RuntimeError::RuntimeFailed
            | RuntimeError::OperationInProgress
            | RuntimeError::NotProvisioned
            | RuntimeError::CatalogUnavailable
            | RuntimeError::PersistenceUnavailable
            | RuntimeError::InvalidTransition => GetRunpodPlacementErrorCode::CommandError,
        })
    }
}
```

In the facade error test, construct `RuntimeError::ProviderUnavailable` and `RuntimeError::InvalidCredential`; keep its expected `CommandError` assertions unchanged.

- [ ] **Step 6: Verify the old lifecycle type is gone and focused tests pass**

Run:

```bash
rg -n "RunpodRuntimeError" src-tauri/src src-tauri/tests
```

Expected: no output and exit status 1.

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::errors::tests
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::errors::tests
cargo test --manifest-path src-tauri/Cargo.toml facade::errors::tests
```

Expected: all selected tests pass and existing command error codes are unchanged.

- [ ] **Step 7: Commit the common error boundary**

```bash
git add src-tauri/src/application/runtimes/errors.rs src-tauri/src/application/runtimes/mod.rs src-tauri/src/application/runtimes/runpod/errors.rs src-tauri/src/application/runtimes/runpod/mod.rs src-tauri/src/application/runtimes/runpod/service.rs src-tauri/src/facade/errors.rs src-tauri/src/facade/state.rs
git commit -m "refactor(runtime): promote lifecycle error"
```

### Task 2: Centralize provider and requirement narrowing

**Files:**

- Modify: `src-tauri/src/application/runtimes/model.rs:26-84,210-290`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs:6-112,282-317,575-623`

**Interfaces:**

- Consumes: `RuntimeProvider`, `RuntimeContractRequirements`, and common `RuntimeError` from Task 1.
- Produces: `RuntimeProvider::as_runpod()`, `RuntimeProvider::as_runpod_mut()`, and `RuntimeContractRequirements::as_runpod()` for provider-specific lifecycle code.

- [ ] **Step 1: Add failing narrowing tests**

Add `RunpodContractRequirements` to the RunPod imports in `model.rs` tests and add:

```rust
#[test]
fn runtime_unions_expose_their_runpod_values() {
    let mut provider = RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
        RunpodRuntimeConfig {
            datacenter_id: "EU-RO-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb: 100,
        },
    ));

    assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 100);
    provider.as_runpod_mut().unwrap().config.volume_size_gb = 120;
    assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 120);

    let expected = RunpodContractRequirements {
        provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
        endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
    };
    let requirements = RuntimeContractRequirements::Runpod(expected.clone());

    assert_eq!(requirements.as_runpod(), Some(&expected));
}
```

In the `runpod/service.rs` test module, import `CatalogRef`, `RuntimeContractRequirements`, `RuntimeError`, and `RunpodContractRequirements`, then add:

```rust
#[test]
fn runpod_requirement_lookup_rejects_a_missing_requirement() {
    let expected = RunpodContractRequirements {
        provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
        endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
    };
    let requirements = vec![RuntimeContractRequirements::Runpod(expected.clone())];

    assert_eq!(super::runpod_requirements(&requirements), Ok(&expected));
    assert_eq!(
        super::runpod_requirements(&[]),
        Err(RuntimeError::CatalogUnavailable)
    );
}
```

- [ ] **Step 2: Run both tests and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml runtime_unions_expose_their_runpod_values
cargo test --manifest-path src-tauri/Cargo.toml runpod_requirement_lookup_rejects_a_missing_requirement
```

Expected: compilation fails because the three narrowing methods and `runpod_requirements` do not exist.

- [ ] **Step 3: Add narrowing methods to the shared enums**

Add directly below `RuntimeContractRequirements` in `src-tauri/src/application/runtimes/model.rs`:

```rust
impl RuntimeContractRequirements {
    pub fn as_runpod(&self) -> Option<&RunpodContractRequirements> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}
```

Extend the existing `RuntimeProvider` implementation to:

```rust
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

- [ ] **Step 4: Use narrowing in the RunPod lifecycle service**

Import `RunpodContractRequirements` in `runpod/service.rs`, then replace the two workspace helpers and add the requirement helper:

```rust
fn runpod(workspace: &Workspace) -> Result<&RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_ref()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod()
        .ok_or(RuntimeError::InvalidTransition)
}

fn runpod_mut(workspace: &mut Workspace) -> Result<&mut RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod_mut()
        .ok_or(RuntimeError::InvalidTransition)
}

fn runpod_requirements(
    requirements: &[RuntimeContractRequirements],
) -> Result<&RunpodContractRequirements, RuntimeError> {
    requirements
        .iter()
        .find_map(RuntimeContractRequirements::as_runpod)
        .ok_or(RuntimeError::CatalogUnavailable)
}
```

Replace the `.first().map(match ...)` block in `start_provision` with:

```rust
let requirements = runpod_requirements(&workflow.contract_requirements)?;
```

Keep `RuntimeProvider::Runpod(...)` where a new shared runtime model is constructed; that is construction, not narrowing.

- [ ] **Step 5: Run the narrowing and RunPod service tests**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml runtime_unions_expose_their_runpod_values
cargo test --manifest-path src-tauri/Cargo.toml runpod_requirement_lookup_rejects_a_missing_requirement
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests
```

Expected: all selected tests pass; existing cleanup, provision, recovery, credential, and transition behavior remains green.

- [ ] **Step 6: Commit narrowing ownership**

```bash
git add src-tauri/src/application/runtimes/model.rs src-tauri/src/application/runtimes/runpod/service.rs
git commit -m "refactor(runpod): centralize runtime narrowing"
```

### Task 3: Add the provider-neutral application lifecycle service

**Files:**

- Create: `src-tauri/src/application/runtimes/service.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs:1-17`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs:10-229`

**Interfaces:**

- Consumes: `RunpodRuntimeService`, `WorkspaceRepository`, `RuntimeOperationRepository`, `RuntimeKind`, `RuntimeOperation`, `Workspace`, and `RuntimeError`.
- Produces: `ProvisionRuntime::Runpod(ProvisionRunpodRuntime)` and `RuntimeService::{new,start_provision,start_cleanup,recover_interrupted}`.

- [ ] **Step 1: Add dispatcher behavior tests before its implementation**

Create `src-tauri/src/application/runtimes/service.rs` with this test module only, and add `mod service;` to `runtimes/mod.rs` so Cargo compiles it:

```rust
#[cfg(test)]
mod tests {
    use crate::application::runtimes::{
        runpod::test_support::{provision_command, ProvisionFakes},
        RuntimeError, RuntimeKind, RuntimeOperationKind, RuntimeOperationState, RuntimeState,
    };

    use super::ProvisionRuntime;

    #[tokio::test]
    async fn provision_dispatches_the_runpod_command() {
        let fakes = ProvisionFakes::ready();
        fakes.block_first_provider_call();

        let (workspace, operation) = fakes
            .runtime_service()
            .start_provision(ProvisionRuntime::Runpod(provision_command()))
            .await
            .unwrap();

        assert_eq!(workspace.runtime.unwrap().kind(), RuntimeKind::Runpod);
        assert_eq!(operation.runtime_kind, RuntimeKind::Runpod);
        fakes.wait_until_first_provider_call().await;
        fakes.release_first_provider_call();
    }

    #[tokio::test]
    async fn cleanup_loads_the_workspace_and_dispatches_by_attached_kind() {
        let fakes = ProvisionFakes::ready_runtime();
        fakes.block_first_provider_call();

        let (workspace, operation) = fakes
            .runtime_service()
            .start_cleanup("workspace-1")
            .await
            .unwrap();

        assert_eq!(workspace.runtime.unwrap().state, RuntimeState::CleaningUp);
        assert_eq!(operation.kind, RuntimeOperationKind::Cleanup);
        fakes.wait_until_first_provider_call().await;
        fakes.release_first_provider_call();
    }

    #[tokio::test]
    async fn cleanup_reports_a_missing_workspace() {
        let fakes = ProvisionFakes::ready_runtime();

        assert_eq!(
            fakes.runtime_service().start_cleanup("missing").await,
            Err(RuntimeError::WorkspaceNotFound)
        );
    }

    #[tokio::test]
    async fn recovery_loads_and_groups_running_operations() {
        let fakes = ProvisionFakes::with_running_provision_and_cleanup();

        fakes.runtime_service().recover_interrupted().await.unwrap();

        assert_eq!(
            fakes.saved_states(),
            vec![
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
            ]
        );
    }
}
```

- [ ] **Step 2: Run the dispatcher tests and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::service::tests
```

Expected: compilation fails because `ProvisionRuntime`, `ProvisionFakes::runtime_service`, and `ProvisionFakes::saved_states` are not defined.

- [ ] **Step 3: Implement the minimal closed application dispatcher**

Add this implementation above the tests in `src-tauri/src/application/runtimes/service.rs`:

```rust
use std::sync::Arc;

use crate::application::workspace::{ports::WorkspaceRepository, Workspace};

use super::{
    ports::RuntimeOperationRepository,
    runpod::{ProvisionRunpodRuntime, RunpodRuntimeService},
    RuntimeError, RuntimeKind, RuntimeOperation,
};

#[derive(crate::diagnostics::DiagnosticDebug)]
pub enum ProvisionRuntime {
    Runpod(#[diagnostic(show)] ProvisionRunpodRuntime),
}

pub struct RuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    operations: Arc<dyn RuntimeOperationRepository>,
    runpod: RunpodRuntimeService,
}

impl RuntimeService {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        operations: Arc<dyn RuntimeOperationRepository>,
        runpod: RunpodRuntimeService,
    ) -> Self {
        Self {
            workspaces,
            operations,
            runpod,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn start_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRuntime,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        match command {
            ProvisionRuntime::Runpod(command) => self.runpod.start_provision(command).await,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn start_cleanup(
        &self,
        #[diagnostic(show)] workspace_id: &str,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        let workspace = self
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|_| RuntimeError::PersistenceUnavailable)?
            .ok_or(RuntimeError::WorkspaceNotFound)?;
        let kind = workspace
            .runtime
            .as_ref()
            .map(|runtime| runtime.kind())
            .ok_or(RuntimeError::NotProvisioned)?;

        match kind {
            RuntimeKind::Runpod => self.runpod.start_cleanup(workspace).await,
        }
    }

    #[crate::diagnostics::diagnostic(show_error)]
    pub async fn recover_interrupted(&self) -> Result<(), RuntimeError> {
        let operations = self.operations.running().await?;
        let mut runpod = Vec::new();
        for operation in operations {
            match operation.runtime_kind {
                RuntimeKind::Runpod => runpod.push(operation),
            }
        }
        self.runpod.recover_interrupted(runpod).await
    }
}
```

In `src-tauri/src/application/runtimes/mod.rs`, add the module and exports:

```rust
mod service;

pub use service::{ProvisionRuntime, RuntimeService};
```

- [ ] **Step 4: Reuse the RunPod fakes from the application-level tests**

Add `RuntimeService` to the `crate::application::runtimes` import in `runpod/test_support.rs`. Add these methods immediately after `ProvisionFakes::service`:

```rust
pub fn runtime_service(&self) -> RuntimeService {
    RuntimeService::new(
        self.workspaces.clone(),
        self.operations.clone(),
        self.service(),
    )
}

pub fn saved_states(&self) -> Vec<(RuntimeState, RuntimeOperationState)> {
    self.repository.saved_states()
}
```

Do not add new fake repository/provider types.

- [ ] **Step 5: Run the application dispatcher and existing RunPod recovery tests**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::service::tests
cargo test --manifest-path src-tauri/Cargo.toml application::runtimes::runpod::service::tests
```

Expected: all selected tests pass; provision and cleanup still return their initial durable snapshots before detached provider work, and all interrupted RunPod operations become failed.

- [ ] **Step 6: Commit application-owned dispatch**

```bash
git add src-tauri/src/application/runtimes/service.rs src-tauri/src/application/runtimes/mod.rs src-tauri/src/application/runtimes/runpod/test_support.rs
git commit -m "refactor(runtime): move lifecycle dispatch into application"
```

### Task 4: Remove facade dispatch and wire the application service

**Files:**

- Modify: `src-tauri/src/facade/state.rs:1-337`
- Modify: `src-tauri/src/lib.rs:20-132`
- Verify unchanged: `src-tauri/src/facade/errors.rs:244-324`
- Verify unchanged: `src/generated/commands.ts`

**Interfaces:**

- Consumes: `ProvisionRuntime` and `RuntimeService` from Task 3, common `RuntimeError` from Task 1, and the existing cloneable `RunpodRuntimeService` for placement.
- Produces: a facade that maps transport input only and a bootstrap graph with application-owned lifecycle dispatch.

- [ ] **Step 1: Replace facade dispatch tests with a failing transport-mapping test**

Replace the complete `#[cfg(test)] mod tests` in `src-tauri/src/facade/state.rs` with:

```rust
#[cfg(test)]
mod tests {
    use crate::application::runtimes::ProvisionRuntime;

    use super::{provision_command, ProvisionRuntimeInput};

    #[test]
    fn provision_input_maps_to_the_application_command() {
        let ProvisionRuntime::Runpod(command) = provision_command(
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
}
```

- [ ] **Step 2: Run the facade test and confirm the red state**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_input_maps_to_the_application_command
```

Expected: compilation fails because `provision_command` does not yet exist; the current helper returns `ProvisionRunpodRuntime` and is named `runpod_provision_command`.

- [ ] **Step 3: Delete `facade::RuntimeDispatcher` and inject both required services**

In `src-tauri/src/facade/state.rs`, replace the complete application import with:

```rust
use crate::application::{
    runtimes::{
        runpod::{ProvisionRunpodRuntime, RunpodRuntimeService},
        ProvisionRuntime, RuntimeError, RuntimeOperationQueryService, RuntimeService,
    },
    secrets::{SecretKind, SecretsService},
    workspace::WorkspaceService,
};
```

Delete the complete `RuntimeDispatcher` struct and implementation. Change `FacadeState` and its constructor to:

```rust
pub struct FacadeState {
    workspaces: WorkspaceService,
    secrets: SecretsService,
    operations: RuntimeOperationQueryService,
    runtimes: RuntimeService,
    runpod: RunpodRuntimeService,
}

impl FacadeState {
    pub fn new(
        workspaces: WorkspaceService,
        secrets: SecretsService,
        operations: RuntimeOperationQueryService,
        runtimes: RuntimeService,
        runpod: RunpodRuntimeService,
    ) -> Self {
        Self {
            workspaces,
            secrets,
            operations,
            runtimes,
            runpod,
        }
    }
}
```

Keep all existing facade methods, but replace the four lifecycle/placement bodies with these calls:

```rust
pub async fn provision_workspace(
    &self,
    request: ProvisionWorkspaceRequest,
) -> CommandResult<WorkspaceOperationDto, ProvisionWorkspaceErrorCode> {
    let (workspace, operation) = self
        .runtimes
        .start_provision(provision_command(&request.workspace_id, request.runtime))
        .await?;
    Ok(WorkspaceOperationDto {
        workspace: workspace.try_into()?,
        operation: operation.try_into()?,
    })
}

pub async fn cleanup_workspace(
    &self,
    request: WorkspaceIdRequest,
) -> CommandResult<WorkspaceOperationDto, CleanupWorkspaceErrorCode> {
    let (workspace, operation) = self.runtimes.start_cleanup(&request.workspace_id).await?;
    Ok(WorkspaceOperationDto {
        workspace: workspace.try_into()?,
        operation: operation.try_into()?,
    })
}

pub async fn get_runpod_placement(
    &self,
) -> CommandResult<RunpodPlacementDto, GetRunpodPlacementErrorCode> {
    Ok(self.runpod.placement().await?.into())
}

pub async fn recover_interrupted(&self) -> Result<(), RuntimeError> {
    self.runtimes.recover_interrupted().await
}
```

Replace `runpod_provision_command` with transport mapping only:

```rust
fn provision_command(workspace_id: &str, input: ProvisionRuntimeInput) -> ProvisionRuntime {
    match input {
        ProvisionRuntimeInput::Runpod {
            datacenter_id,
            gpu_id,
            volume_size_gb,
        } => ProvisionRuntime::Runpod(ProvisionRunpodRuntime {
            workspace_id: workspace_id.to_owned(),
            datacenter_id,
            gpu_id,
            volume_size_gb,
        }),
    }
}
```

Delete `attached_runtime_kind` and its two facade tests. Remove now-unused `Workspace`, `RuntimeKind`, and `RuntimeOperation` imports.

- [ ] **Step 4: Wire shared repositories and both service views in bootstrap**

In `src-tauri/src/lib.rs`, import `RuntimeService` and remove `RuntimeDispatcher`:

```rust
use application::{
    runtimes::{
        runpod::{RunpodRuntimeService, RunpodRuntimeServiceDependencies},
        RuntimeOperationQueryService, RuntimeService,
    },
    secrets::SecretsService,
    workspace::WorkspaceService,
};
use facade::{FacadeState, TauriEventSink};
```

Replace service construction from `operations_service` through `FacadeState::new` with:

```rust
let operations_service = RuntimeOperationQueryService::new(operations.clone());
let runpod_service = RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
    workspaces: workspaces.clone(),
    workflows: bundled.clone(),
    transitions,
    runtime_catalog: bundled,
    secrets,
    provider: runpod_provider,
    events,
});
let runtime_service = RuntimeService::new(workspaces, operations, runpod_service.clone());
let facade_state = FacadeState::new(
    workspace_service,
    secrets_service,
    operations_service,
    runtime_service,
    runpod_service,
);
```

Keep event mounting before `facade_state.recover_interrupted()` and keep the existing bootstrap error mapping.

- [ ] **Step 5: Run the facade test and ownership checks**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_input_maps_to_the_application_command
rg -n "RuntimeDispatcher|attached_runtime_kind|runpod_provision_command" src-tauri/src
git diff --exit-code -- src/generated/commands.ts
```

Expected: the test passes; the ownership search prints nothing and exits 1; generated bindings have no diff and the final command exits 0.

- [ ] **Step 6: Run full native verification**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: all tests pass, formatting is clean, and Clippy reports no warnings.

- [ ] **Step 7: Inspect the final scope and commit the facade wiring**

Run:

```bash
git status --short
git diff --stat HEAD
```

Expected: only the files listed in Tasks 1-4 are changed since the implementation started; no adapter, infra, migration, generated, or frontend file appears.

Commit:

```bash
git add src-tauri/src/facade/state.rs src-tauri/src/lib.rs
git commit -m "refactor(runtime): route lifecycle through application"
```

The resulting boundary is intentionally closed: adding a future runtime extends the common enums, adds one provider-specific service and persistence module, adds one application dispatch arm, and adds transport mapping for its provision input. Facade lifecycle orchestration and existing RunPod workflow bodies remain unchanged.
