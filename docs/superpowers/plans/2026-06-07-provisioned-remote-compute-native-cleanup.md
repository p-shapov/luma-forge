# Provisioned Remote Compute Native Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename and clarify the current RunPod-like native runtime as provisioned remote compute, split the oversized service into focused internal modules, and restore minimal typed command errors.

**Architecture:** Preserve current command behavior and runtime lifecycle while making the runtime family explicit. Keep `ProvisionedRemoteComputeService` as the public facade and move provisioning flow, cleanup, contract resolution, and coordination into private sibling modules. Add stable command error codes with UI-safe messages and regenerate TypeScript bindings.

**Tech Stack:** Rust 2021, Tauri 2, Specta/Tauri Specta, sqlx SQLite, reqwest, keyring, Bun/Vite/TypeScript.

---

## File Structure

Create:

- `src-tauri/src/provisioned_remote_compute/flow.rs`: normal provisioning step logic moved out of the service facade.
- `src-tauri/src/provisioned_remote_compute/cleanup.rs`: cancellation and cleanup logic moved out of the service facade.
- `src-tauri/src/provisioned_remote_compute/contracts.rs`: provisioner and endpoint image reference resolution.
- `src-tauri/src/provisioned_remote_compute/coordination.rs`: in-flight workspace guard currently embedded in `service.rs`.

Rename:

- `src-tauri/src/remote_workspace/` -> `src-tauri/src/provisioned_remote_compute/`

Modify:

- `src-tauri/src/domain/workspace.rs`: rename runtime variant and related remote workspace types.
- `src-tauri/src/domain/mod.rs`: update module references if needed.
- `src-tauri/src/provisioned_remote_compute/mod.rs`: expose new internal module layout.
- `src-tauri/src/provisioned_remote_compute/service.rs`: keep public facade and delegate to internal modules.
- `src-tauri/src/provisioned_remote_compute/provider.rs`: rename provider traits and params from remote workspace terminology to provisioned remote compute terminology.
- `src-tauri/src/provisioned_remote_compute/registry.rs`: rename registry type and tests.
- `src-tauri/src/provisioned_remote_compute/providers/runpod/*`: update imports and trait names.
- `src-tauri/src/app/bootstrap.rs`: use renamed service/registry/provider modules.
- `src-tauri/src/app/state.rs`: use renamed service type.
- `src-tauri/src/commands/catalog.rs`: update imports and error mapping.
- `src-tauri/src/commands/workspaces.rs`: update imports and runtime type references.
- `src-tauri/src/commands/types/workspace.rs`: update DTO names, runtime variant, and tests.
- `src-tauri/src/commands/mod.rs`: add `NativeCommandErrorCode` and map errors to `{ code, message }`.
- `src-tauri/src/lib.rs`: rename exported module from `remote_workspace` to `provisioned_remote_compute`.
- `src/generated/commands.ts`: regenerate through `bun run codegen:commands`.

Do not modify:

- Worker code.
- SQLite table schema, persistence versioning, or migration behavior. Persisted workspace JSON follows the current domain contract in this pre-v1 refactor, so old local development workspace rows may need to be recreated.
- Endpoint execution behavior.
- RunPod endpoint cleanup strategy.

---

### Task 1: Rename Runtime Variant And Domain Types

**Files:**

- Modify: `src-tauri/src/domain/workspace.rs`
- Modify: `src-tauri/src/commands/types/workspace.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`
- Modify: `src-tauri/src/workspace_catalog/service.rs`
- Modify: `src-tauri/src/workspace_catalog/sqlite.rs`

- [ ] **Step 1: Write failing domain serialization tests**

Add or update tests in `src-tauri/src/commands/types/workspace.rs`:

```rust
#[test]
fn workspace_runtime_response_serializes_provisioned_remote_compute_variant() {
    let response = WorkspaceRuntimeResponse::ProvisionedRemoteCompute(
        ProvisionedRemoteComputeWorkspaceResponse {
            remote_placement: crate::commands::types::placement::RemotePlacementPlanInput {
                gpu_cloud_provider_id: crate::commands::types::provider::GpuCloudProviderIdDto::Runpod,
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_size_bytes: 1,
                keep_alive_limits: None,
            },
            provisioning: ProvisionedRemoteComputeProvisioningStateResponse {
                status: ProvisionedRemoteComputeProvisioningStatusResponse::NotStarted,
                percent: None,
            },
            resources: ProvisionedRemoteComputeResourcesResponse {
                volume: None,
                provisioner: None,
                endpoint: None,
            },
        },
    );

    let json = serde_json::to_string(&response).expect("runtime json");

    assert!(json.contains(r#""runtimeType":"provisioned_remote_compute""#));
    assert!(!json.contains("remote_provisioner"));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_runtime_response_serializes_provisioned_remote_compute_variant
```

Expected: FAIL because `WorkspaceRuntimeResponse::ProvisionedRemoteCompute` and the new DTO names do not exist yet.

- [ ] **Step 3: Rename domain types in `workspace.rs`**

Change these type names:

```rust
RemoteProvisioningError -> ProvisionedRemoteComputeProvisioningError
RemoteVolumeSnapshot -> ProvisionedRemoteComputeVolumeSnapshot
RemoteProvisionerSnapshot -> ProvisionedRemoteComputeProvisionerSnapshot
RemoteEndpointSnapshot -> ProvisionedRemoteComputeEndpointSnapshot
RemoteProvisionerStatus -> ProvisionedRemoteComputeProvisionerStatus
RemoteProvisioningPhase -> ProvisionedRemoteComputeProvisioningPhase
RemoteProvisioningStatus -> ProvisionedRemoteComputeProvisioningStatus
RemoteProvisioningState -> ProvisionedRemoteComputeProvisioningState
RemoteWorkspaceResources -> ProvisionedRemoteComputeResources
RemoteWorkspace -> ProvisionedRemoteComputeWorkspace
WorkspaceRuntime::Remote -> WorkspaceRuntime::ProvisionedRemoteCompute
```

The final runtime enum in `src-tauri/src/domain/workspace.rs` should have this shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkspaceRuntime {
    ProvisionedRemoteCompute(ProvisionedRemoteComputeWorkspace),
}
```

Keep the existing field names inside `ProvisionedRemoteComputeWorkspace` only where they remain product-language clear. Prefer:

```rust
pub struct ProvisionedRemoteComputeWorkspace {
    pub remote_placement: RemotePlacementPlan,
    pub provisioning: ProvisionedRemoteComputeProvisioningState,
    pub resources: ProvisionedRemoteComputeResources,
}
```

- [ ] **Step 4: Update DTO names in `commands/types/workspace.rs`**

Rename response DTOs to match the domain terms:

```rust
WorkspaceRuntimeResponse::Remote -> WorkspaceRuntimeResponse::ProvisionedRemoteCompute
RemoteWorkspaceResponse -> ProvisionedRemoteComputeWorkspaceResponse
RemoteProvisioningStateResponse -> ProvisionedRemoteComputeProvisioningStateResponse
RemoteProvisioningStatusResponse -> ProvisionedRemoteComputeProvisioningStatusResponse
RemoteProvisioningPhaseResponse -> ProvisionedRemoteComputeProvisioningPhaseResponse
RemoteProvisionerStatusResponse -> ProvisionedRemoteComputeProvisionerStatusResponse
RemoteProvisioningErrorResponse -> ProvisionedRemoteComputeProvisioningErrorResponse
RemoteWorkspaceResourcesResponse -> ProvisionedRemoteComputeResourcesResponse
RemoteVolumeSnapshotResponse -> ProvisionedRemoteComputeVolumeSnapshotResponse
RemoteProvisionerSnapshotResponse -> ProvisionedRemoteComputeProvisionerSnapshotResponse
RemoteEndpointSnapshotResponse -> ProvisionedRemoteComputeEndpointSnapshotResponse
```

The runtime DTO enum should have this serde tag:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum WorkspaceRuntimeResponse {
    ProvisionedRemoteCompute(ProvisionedRemoteComputeWorkspaceResponse),
}
```

- [ ] **Step 5: Update references across native code**

Use search-driven edits:

```bash
rg "WorkspaceRuntime::Remote|RemoteWorkspace|RemoteProvisioning|RemoteProvisioner|RemoteEndpoint|RemoteVolume" src-tauri/src
```

For each match, update to the new provisioned remote compute type names from Step 3 and Step 4. Keep `RemotePlacementPlan`, `RemotePlacementOptions`, and `GpuCloudProviderId` unchanged because they still describe provider placement, not the runtime family.

- [ ] **Step 6: Run tests for the renamed runtime**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_runtime_response_serializes_provisioned_remote_compute_variant
cargo test --manifest-path src-tauri/Cargo.toml setup_workspace_returns_remote_runtime_with_not_started_state
```

Expected: PASS after updating test names/assertions to the new runtime language.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/domain/workspace.rs src-tauri/src/commands/types/workspace.rs src-tauri/src/remote_workspace/service.rs src-tauri/src/workspace_catalog/service.rs src-tauri/src/workspace_catalog/sqlite.rs
git commit -m "refactor(native): rename provisioned remote compute runtime"
```

---

### Task 2: Rename Runtime Module And Provider Boundary

**Files:**

- Rename: `src-tauri/src/remote_workspace` -> `src-tauri/src/provisioned_remote_compute`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/app/bootstrap.rs`
- Modify: `src-tauri/src/app/state.rs`
- Modify: `src-tauri/src/commands/catalog.rs`
- Modify: `src-tauri/src/commands/workspaces.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/provider.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/registry.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/providers/runpod/*`

- [ ] **Step 1: Write failing registry naming test**

In `src-tauri/src/provisioned_remote_compute/registry.rs` after the file is moved, keep or add:

```rust
#[test]
fn missing_provisioned_remote_compute_provider_returns_explicit_error() {
    let registry = ProvisionedRemoteComputeProviderRegistry::empty();

    let error = match registry.for_provider(GpuCloudProviderId::Runpod) {
        Ok(provider) => panic!(
            "missing provider should fail, resolved {:?}",
            provider.provider_id()
        ),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ProvisionedRemoteComputeError::ProviderUnavailable {
            provider_id: GpuCloudProviderId::Runpod
        }
    );
}
```

- [ ] **Step 2: Move module directory**

Run:

```bash
git mv src-tauri/src/remote_workspace src-tauri/src/provisioned_remote_compute
```

- [ ] **Step 3: Rename public module export**

In `src-tauri/src/lib.rs`, replace:

```rust
pub mod remote_workspace;
```

with:

```rust
pub mod provisioned_remote_compute;
```

- [ ] **Step 4: Rename module-level errors and service types**

Apply these type renames in `src-tauri/src/provisioned_remote_compute/**` and callers:

```text
RemoteWorkspaceError -> ProvisionedRemoteComputeError
RemoteWorkspaceService -> ProvisionedRemoteComputeService
SetupWorkspaceRequest -> SetupProvisionedRemoteComputeWorkspaceRequest
RemoteWorkspaceProvider -> ProvisionedRemoteComputeProvider
RemoteWorkspaceProviderRegistry -> ProvisionedRemoteComputeProviderRegistry
RemotePlacementOptionsProvider -> ProvisionedRemoteComputePlacementOptionsProvider
RemoteVolumeProvider -> ProvisionedRemoteComputeVolumeProvider
RemoteProvisionerProvider -> ProvisionedRemoteComputeProvisionerProvider
RemoteEndpointProvider -> ProvisionedRemoteComputeEndpointProvider
```

Keep operation param names concise:

```text
CreateVolumeParams
DeleteVolumeParams
StartProvisionerParams
TerminateProvisionerParams
GetProvisionerStatusParams
CreateEndpointParams
DeleteEndpointParams
```

- [ ] **Step 5: Update RunPod provider type names**

In `src-tauri/src/provisioned_remote_compute/providers/runpod/mod.rs`, rename:

```text
RunpodRemoteWorkspaceProvider -> RunpodProvisionedRemoteComputeProvider
```

Update app bootstrap construction to use:

```rust
let runpod_provider = RunpodProvisionedRemoteComputeProvider::new(
    provider_runpod_secrets,
    provider_hugging_face_secrets,
);
let provider_registry =
    ProvisionedRemoteComputeProviderRegistry::new(vec![Box::new(runpod_provider)]);
let remote_workspace =
    ProvisionedRemoteComputeService::new(provider_registry, workflow_catalog.clone());
```

Keep the `AppState` field name if desired for compatibility within command code, or rename it to:

```rust
pub provisioned_remote_compute: ProvisionedRemoteComputeService,
```

If renaming the field, update all command callers in the same task.

- [ ] **Step 6: Update imports**

Run:

```bash
rg "remote_workspace|RemoteWorkspace|RunpodRemoteWorkspace|ProvisionedRemoteCompute" src-tauri/src
```

Expected after edits: no `remote_workspace` module path remains. `RemotePlacement*` may remain.

- [ ] **Step 7: Run module rename tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml missing_provisioned_remote_compute_provider_returns_explicit_error
cargo test --manifest-path src-tauri/Cargo.toml with_provider_resolves_runpod_provider
```

Expected: PASS with updated test names and type names.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src src-tauri/Cargo.toml
git commit -m "refactor(native): rename provisioned remote compute module"
```

---

### Task 3: Extract Coordination Guard

**Files:**

- Create: `src-tauri/src/provisioned_remote_compute/coordination.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/mod.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/service.rs`

- [ ] **Step 1: Move coordinator tests first**

Create `src-tauri/src/provisioned_remote_compute/coordination.rs` with this test scaffold and no implementation yet:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_enter_rejects_duplicate_workspace_until_guard_drops() {
        let coordinator = ProvisionedRemoteComputeCoordinator::default();

        let first = coordinator.try_enter("workspace-1");
        assert!(first.is_some());
        assert!(coordinator.try_enter("workspace-1").is_none());

        drop(first);

        assert!(coordinator.try_enter("workspace-1").is_some());
    }
}
```

- [ ] **Step 2: Run the failing coordinator test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml try_enter_rejects_duplicate_workspace_until_guard_drops
```

Expected: FAIL because `ProvisionedRemoteComputeCoordinator` is not defined in the new module yet.

- [ ] **Step 3: Move coordinator implementation**

Move the existing coordinator and guard implementation from `service.rs` into `coordination.rs`. The final public-in-crate shape should be:

```rust
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(crate) struct ProvisionedRemoteComputeCoordinator {
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl ProvisionedRemoteComputeCoordinator {
    pub(crate) fn try_enter(&self, workspace_id: &str) -> Option<ProvisionedRemoteComputeGuard> {
        let mut in_flight = self.in_flight.lock().expect("coordinator lock should succeed");
        if !in_flight.insert(workspace_id.to_string()) {
            return None;
        }

        Some(ProvisionedRemoteComputeGuard {
            workspace_id: workspace_id.to_string(),
            in_flight: Arc::clone(&self.in_flight),
        })
    }
}

pub(crate) struct ProvisionedRemoteComputeGuard {
    workspace_id: String,
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl Drop for ProvisionedRemoteComputeGuard {
    fn drop(&mut self) {
        self.in_flight
            .lock()
            .expect("coordinator lock should succeed")
            .remove(&self.workspace_id);
    }
}
```

- [ ] **Step 4: Register module and update service import**

In `src-tauri/src/provisioned_remote_compute/mod.rs`, add:

```rust
mod coordination;
```

In `service.rs`, replace the embedded coordinator definitions with:

```rust
use super::coordination::ProvisionedRemoteComputeCoordinator;
```

The service field should be:

```rust
coordinator: ProvisionedRemoteComputeCoordinator,
```

- [ ] **Step 5: Run coordinator and provisioning conflict tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml try_enter_rejects_duplicate_workspace_until_guard_drops
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_rejects_duplicate_in_flight_workspace_without_provider_calls
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/provisioned_remote_compute/coordination.rs src-tauri/src/provisioned_remote_compute/mod.rs src-tauri/src/provisioned_remote_compute/service.rs
git commit -m "refactor(native): extract provisioned compute coordination"
```

---

### Task 4: Extract Contract Image Resolution

**Files:**

- Create: `src-tauri/src/provisioned_remote_compute/contracts.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/mod.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/service.rs`

- [ ] **Step 1: Write failing contract resolver tests**

Create `src-tauri/src/provisioned_remote_compute/contracts.rs` and move or recreate tests for provisioner and endpoint contract resolution. The test names should be:

```rust
#[test]
fn resolve_provisioner_image_ref_returns_bundled_image_ref()
```

```rust
#[test]
fn resolve_endpoint_image_ref_returns_bundled_image_ref()
```

Each test should build the existing fake/default workflow catalog service used by current `service.rs` tests and assert the same image refs currently asserted in provisioning tests.

Use these expected image refs from current tests:

```rust
const EXPECTED_ENDPOINT_IMAGE_REF: &str =
    "ghcr.io/p-shapov/luma-forge/runpod-endpoint-worker@sha256:ac7b4ee14423f5e74f444a03c429dece830fc4f72b01847df18b2a5b960cdd1a";

const EXPECTED_PROVISIONER_IMAGE_REF: &str =
    "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:8e0d74276a36db8b0fae428b492e8fd080eea5311a7d153a0d60023c7e5a8295";
```

- [ ] **Step 2: Run failing contract resolver tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml resolve_provisioner_image_ref_returns_bundled_image_ref
cargo test --manifest-path src-tauri/Cargo.toml resolve_endpoint_image_ref_returns_bundled_image_ref
```

Expected: FAIL until resolver functions are moved and exposed to tests.

- [ ] **Step 3: Move resolver code**

Move these functions from `service.rs` to `contracts.rs`:

```text
resolve_provisioner_image_ref
resolve_endpoint_image_ref
resolve_runtime_contract_reference
```

Represent them as a small resolver type:

```rust
use crate::{
    domain::{
        runtime_contract::RuntimeContractReference,
        workflow_preset::RemoteProviderRuntimeRequirements,
        workspace::{
            ProvisionedRemoteComputeProvisioningError, ProvisionedRemoteComputeWorkspace, Workspace,
        },
    },
    workflow_catalog::WorkflowCatalogService,
};

pub(crate) struct ProvisionedRemoteComputeContractResolver<'a> {
    workflow_catalog_service: &'a WorkflowCatalogService,
}

impl<'a> ProvisionedRemoteComputeContractResolver<'a> {
    pub(crate) fn new(workflow_catalog_service: &'a WorkflowCatalogService) -> Self {
        Self {
            workflow_catalog_service,
        }
    }

    pub(crate) fn provisioner_image_ref(
        &self,
        workspace: &Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract = self.runtime_contract_reference(workspace, runtime, |requirements| {
            &requirements.provisioner_contract
        })?;
        let catalog = self
            .workflow_catalog_service
            .get_provisioner_contract_catalog()
            .map_err(|error| ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!("provisioner contract catalog is invalid: {error:?}"),
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "provisioner contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    pub(crate) fn endpoint_image_ref(
        &self,
        workspace: &Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
    ) -> Result<String, ProvisionedRemoteComputeProvisioningError> {
        let contract = self.runtime_contract_reference(workspace, runtime, |requirements| {
            &requirements.endpoint_contract
        })?;
        let catalog = self
            .workflow_catalog_service
            .get_endpoint_contract_catalog()
            .map_err(|error| ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!("endpoint contract catalog is invalid: {error:?}"),
            })?;
        let resolved = catalog.resolve(contract).ok_or_else(|| {
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "endpoint contract is not bundled: {}@{}",
                    contract.id, contract.version
                ),
            }
        })?;

        Ok(resolved.image_ref)
    }

    fn runtime_contract_reference<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &ProvisionedRemoteComputeWorkspace,
        contract: impl FnOnce(&'w RemoteProviderRuntimeRequirements) -> &'w RuntimeContractReference,
    ) -> Result<&'w RuntimeContractReference, ProvisionedRemoteComputeProvisioningError> {
        let provider_requirements = workspace
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(runtime.remote_placement.gpu_cloud_provider_id)
            .ok_or_else(|| ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!(
                    "workflow preset has no runtime requirements for provider {:?}",
                    runtime.remote_placement.gpu_cloud_provider_id
                ),
            })?;

        Ok(contract(provider_requirements))
    }
}
```

- [ ] **Step 4: Update service calls**

In `service.rs`, replace direct resolver methods with:

```rust
let resolver = ProvisionedRemoteComputeContractResolver::new(&self.workflow_catalog_service);
let provisioner_image_ref = match resolver.provisioner_image_ref(workspace, runtime) {
    Ok(image_ref) => image_ref,
    Err(error) => {
        return Ok(with_provisioning_failure(workspace, Some(phase.clone()), error));
    }
};
```

and:

```rust
let resolver = ProvisionedRemoteComputeContractResolver::new(&self.workflow_catalog_service);
let endpoint_image_ref = match resolver.endpoint_image_ref(workspace, runtime) {
    Ok(image_ref) => image_ref,
    Err(error) => {
        return Ok(with_provisioning_failure(workspace, Some(phase.clone()), error));
    }
};
```

- [ ] **Step 5: Register module**

In `src-tauri/src/provisioned_remote_compute/mod.rs`, add:

```rust
mod contracts;
```

- [ ] **Step 6: Run contract and provisioning tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml resolve_provisioner_image_ref_returns_bundled_image_ref
cargo test --manifest-path src-tauri/Cargo.toml resolve_endpoint_image_ref_returns_bundled_image_ref
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_starting_provisioner_advances_one_step
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_creating_endpoint_marks_completed
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/provisioned_remote_compute/contracts.rs src-tauri/src/provisioned_remote_compute/mod.rs src-tauri/src/provisioned_remote_compute/service.rs
git commit -m "refactor(native): extract provisioned compute contract resolution"
```

---

### Task 5: Extract Provisioning Flow

**Files:**

- Create: `src-tauri/src/provisioned_remote_compute/flow.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/mod.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/service.rs`

- [ ] **Step 1: Move existing flow tests into a flow-focused module**

Move the existing normal provisioning tests from `service.rs` into `flow.rs` or keep them in `service.rs` while extracting implementation. The required test names are:

```text
provision_workspace_not_started_marks_creating_volume_without_provider_calls
provision_workspace_creating_volume_creates_volume_only
provision_workspace_starting_provisioner_advances_one_step
provision_workspace_running_provisioner_stores_incomplete_status
provision_workspace_worker_success_moves_to_cleanup
provision_workspace_cleanup_after_success_moves_to_endpoint_creation
provision_workspace_creating_endpoint_marks_completed
```

- [ ] **Step 2: Run baseline flow tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_not_started_marks_creating_volume_without_provider_calls
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_creating_volume_creates_volume_only
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_creating_endpoint_marks_completed
```

Expected: PASS before extraction. If any fail, stop and fix the current branch before refactoring.

- [ ] **Step 3: Move normal provisioning handlers**

Move these methods from `service.rs` to `flow.rs` as free functions or as methods on a private flow struct:

```text
handle_not_started
handle_creating_volume
handle_starting_provisioner
handle_running_provisioner
handle_cleaning_up_provisioner
handle_creating_endpoint
handle_terminal_status
```

Use this context struct to avoid a long argument list:

```rust
use crate::workflow_catalog::WorkflowCatalogService;

use super::provider::ProvisionedRemoteComputeProvider;

pub(crate) struct ProvisionedRemoteComputeFlowContext<'a> {
    pub(crate) workflow_catalog_service: &'a WorkflowCatalogService,
    pub(crate) provider: &'a dyn ProvisionedRemoteComputeProvider,
}
```

The main dispatch in `service.rs::provision_workspace` should become a small match that delegates to `flow`:

```rust
let flow_context = ProvisionedRemoteComputeFlowContext {
    workflow_catalog_service: &self.workflow_catalog_service,
    provider,
};

match &runtime.provisioning.status {
    ProvisionedRemoteComputeProvisioningStatus::NotStarted => {
        flow::handle_not_started(workspace)
    }
    ProvisionedRemoteComputeProvisioningStatus::InProgress {
        phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
    } => {
        flow::handle_creating_volume(workspace, runtime, &flow_context).await
    }
    ProvisionedRemoteComputeProvisioningStatus::InProgress {
        phase: phase @ ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
    } => {
        flow::handle_starting_provisioner(workspace, runtime, &flow_context, phase).await
    }
    ProvisionedRemoteComputeProvisioningStatus::InProgress {
        phase:
            phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
            },
    } => {
        flow::handle_cleaning_up_provisioner(workspace, runtime, &flow_context, phase).await
    }
    ProvisionedRemoteComputeProvisioningStatus::InProgress {
        phase: phase @ ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { .. },
    } => {
        flow::handle_running_provisioner(workspace, runtime, &flow_context, phase).await
    }
    ProvisionedRemoteComputeProvisioningStatus::InProgress {
        phase: phase @ ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
    } => {
        flow::handle_creating_endpoint(workspace, runtime, &flow_context, phase).await
    }
    ProvisionedRemoteComputeProvisioningStatus::Completed
    | ProvisionedRemoteComputeProvisioningStatus::Failed { .. } => {
        flow::handle_terminal_status(workspace)
    }
    ProvisionedRemoteComputeProvisioningStatus::Cancelling { phase } => {
        cleanup::handle_cancelling(workspace, runtime, provider, phase.clone()).await
    }
}
```

Use the exact renamed phase variants from Task 1.

- [ ] **Step 4: Register module**

In `src-tauri/src/provisioned_remote_compute/mod.rs`, add:

```rust
mod flow;
```

- [ ] **Step 5: Run full provisioning flow tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_
```

Expected: all tests whose names start with `provision_workspace_` pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/provisioned_remote_compute/flow.rs src-tauri/src/provisioned_remote_compute/mod.rs src-tauri/src/provisioned_remote_compute/service.rs
git commit -m "refactor(native): extract provisioned compute flow"
```

---

### Task 6: Extract Cleanup And Cancellation

**Files:**

- Create: `src-tauri/src/provisioned_remote_compute/cleanup.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/mod.rs`
- Modify: `src-tauri/src/provisioned_remote_compute/service.rs`

- [ ] **Step 1: Identify cleanup tests to preserve**

Keep these tests passing through extraction:

```text
cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls
cancel_workspace_not_started_marks_invalid_state_without_provider_calls
provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase
provision_workspace_cancelling_terminates_provisioner_without_polling_status
provision_workspace_cancelling_deletes_volume_and_resets_to_not_started
cleanup_workspace_cleans_resources_in_dependency_order
cleanup_workspace_ignores_not_found_cleanup_errors
cleanup_workspace_endpoint_cleanup_failure_marks_failed_and_stops_cleanup
cleanup_workspace_provisioner_cleanup_failure_marks_failed_and_stops_cleanup
cleanup_workspace_volume_cleanup_failure_marks_failed
```

- [ ] **Step 2: Run baseline cleanup tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls
cargo test --manifest-path src-tauri/Cargo.toml cleanup_workspace_cleans_resources_in_dependency_order
```

Expected: PASS before extraction.

- [ ] **Step 3: Move cancellation and cleanup handlers**

Move these functions/methods from `service.rs` to `cleanup.rs`:

```text
handle_cancelling
cancel_workspace logic body
cleanup_workspace logic body
```

Keep the public methods on `ProvisionedRemoteComputeService`:

```rust
pub fn cancel_workspace(
    &self,
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    cleanup::mark_cancelling(workspace)
}

pub async fn cleanup_workspace(
    &self,
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let runtime = provisioned_remote_compute_runtime(workspace)?;
    let provider_id = runtime.remote_placement.gpu_cloud_provider_id;
    let provider = self.provider_registry.for_provider(provider_id)?;

    cleanup::cleanup_workspace(workspace, runtime, provider).await
}
```

For in-progress cancellation during provisioning, call:

```rust
cleanup::handle_cancelling(workspace, runtime, provider, phase.clone()).await
```

- [ ] **Step 4: Register module**

In `src-tauri/src/provisioned_remote_compute/mod.rs`, add:

```rust
mod cleanup;
```

- [ ] **Step 5: Run cleanup and cancellation tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cancel_workspace_
cargo test --manifest-path src-tauri/Cargo.toml cleanup_workspace_
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_
```

Expected: all matching tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/provisioned_remote_compute/cleanup.rs src-tauri/src/provisioned_remote_compute/mod.rs src-tauri/src/provisioned_remote_compute/service.rs
git commit -m "refactor(native): extract provisioned compute cleanup"
```

---

### Task 7: Add Minimal Typed Command Errors

**Files:**

- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/types/workspace.rs`
- Test through existing command type tests and binding export test.

- [ ] **Step 1: Write failing error serialization tests**

In `src-tauri/src/commands/mod.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_command_error_serializes_code_and_message() {
        let error = NativeCommandError::new(
            NativeCommandErrorCode::WorkspaceNotFound,
            "workspace was not found",
        );

        let json = serde_json::to_string(&error).expect("command error json");

        assert_eq!(
            json,
            r#"{"code":"workspace_not_found","message":"workspace was not found"}"#
        );
    }
}
```

- [ ] **Step 2: Run failing error test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml native_command_error_serializes_code_and_message
```

Expected: FAIL because `NativeCommandErrorCode` does not exist yet.

- [ ] **Step 3: Add command error code enum**

In `src-tauri/src/commands/mod.rs`, replace the current error struct with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    WorkflowCatalogInvalid,
    WorkspaceStorageUnavailable,
    WorkspaceStorageQueryFailed,
    WorkspaceStorageCorrupt,
    WorkspaceStorageSchemaMismatch,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
    ProviderUnavailable,
    ProviderSecretUnavailable,
    ProviderUnauthorized,
    ProviderInsufficientPermissions,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed,
    ProvisioningAlreadyRunning,
    InvalidProvisioningState,
    ProvisionerWorkerUnauthorized,
    ProvisionerWorkerUnavailable,
    ProvisionerWorkerConflict,
    ProvisionerWorkerResponseInvalid,
    ProvisionerWorkerFailed,
    CommandNotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
}

impl NativeCommandError {
    pub fn new(code: NativeCommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
```

- [ ] **Step 4: Update workflow catalog error mapping**

Replace:

```rust
Self::new("workflow catalog could not be read")
```

with:

```rust
Self::new(
    NativeCommandErrorCode::WorkflowCatalogInvalid,
    "workflow catalog could not be read",
)
```

Map both `WorkflowCatalogError::ParseFailed` and `WorkflowCatalogError::ValidationFailed` to `WorkflowCatalogInvalid` with their existing UI-safe messages.

- [ ] **Step 5: Update workspace catalog error mapping**

Map current workspace catalog errors:

```rust
WorkspaceCatalogError::StorageUnavailable => NativeCommandErrorCode::WorkspaceStorageUnavailable
WorkspaceCatalogError::MigrationFailed => NativeCommandErrorCode::WorkspaceStorageUnavailable
WorkspaceCatalogError::QueryFailed => NativeCommandErrorCode::WorkspaceStorageQueryFailed
WorkspaceCatalogError::Corrupt => NativeCommandErrorCode::WorkspaceStorageCorrupt
WorkspaceCatalogError::SchemaMismatch => NativeCommandErrorCode::WorkspaceStorageSchemaMismatch
WorkspaceCatalogError::WorkspaceAlreadyExists => NativeCommandErrorCode::WorkspaceAlreadyExists
WorkspaceCatalogError::WorkspaceNotFound => NativeCommandErrorCode::WorkspaceNotFound
```

Keep existing messages.

- [ ] **Step 6: Update secrets storage error mapping**

Map current secrets errors:

```rust
SecretsStorageError::SecretRequired => NativeCommandErrorCode::ProviderSecretUnavailable
SecretsStorageError::KeyAlreadyExists => NativeCommandErrorCode::ProviderRequestFailed
SecretsStorageError::KeyNotFound => NativeCommandErrorCode::ProviderSecretUnavailable
SecretsStorageError::StoreUnavailable => NativeCommandErrorCode::ProviderRequestFailed
SecretsStorageError::StoredSecretInvalid => NativeCommandErrorCode::ProviderRequestFailed
SecretsStorageError::Provider(_) => NativeCommandErrorCode::ProviderRequestFailed
SecretsStorageError::IdentityResponseInvalid => NativeCommandErrorCode::ProviderRequestFailed
```

Keep existing messages. Do not expose raw provider details.

- [ ] **Step 7: Update provisioned remote compute error mapping**

Map current runtime errors:

```rust
ProvisionedRemoteComputeError::SetupWorkspaceInvalidRequest { .. } => NativeCommandErrorCode::InvalidProvisioningState
ProvisionedRemoteComputeError::ProviderUnavailable { .. } => NativeCommandErrorCode::ProviderUnavailable
ProvisionedRemoteComputeError::ProviderSecretUnavailable => NativeCommandErrorCode::ProviderSecretUnavailable
ProvisionedRemoteComputeError::ProvisioningAlreadyRunning { .. } => NativeCommandErrorCode::ProvisioningAlreadyRunning
ProvisionedRemoteComputeError::Provider(ProviderApiError::Unauthorized) => NativeCommandErrorCode::ProviderUnauthorized
ProvisionedRemoteComputeError::Provider(ProviderApiError::InsufficientPermissions) => NativeCommandErrorCode::ProviderInsufficientPermissions
ProvisionedRemoteComputeError::Provider(ProviderApiError::RateLimited) => NativeCommandErrorCode::ProviderRateLimited
ProvisionedRemoteComputeError::Provider(ProviderApiError::Timeout) => NativeCommandErrorCode::ProviderTimeout
ProvisionedRemoteComputeError::Provider(ProviderApiError::RequestFailed { .. }) => NativeCommandErrorCode::ProviderRequestFailed
ProvisionedRemoteComputeError::RemoteVolumeNotFound => NativeCommandErrorCode::ProviderRequestFailed
ProvisionedRemoteComputeError::RemoteProvisionerNotFound => NativeCommandErrorCode::ProviderRequestFailed
ProvisionedRemoteComputeError::RemoteEndpointNotFound => NativeCommandErrorCode::ProviderRequestFailed
ProvisionedRemoteComputeError::ProvisionerWorker(ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnauthorized) => NativeCommandErrorCode::ProvisionerWorkerUnauthorized
ProvisionedRemoteComputeError::ProvisionerWorker(ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnavailable) => NativeCommandErrorCode::ProvisionerWorkerUnavailable
ProvisionedRemoteComputeError::ProvisionerWorker(ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerConflict) => NativeCommandErrorCode::ProvisionerWorkerConflict
ProvisionedRemoteComputeError::ProvisionerWorker(ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerResponseInvalid) => NativeCommandErrorCode::ProvisionerWorkerResponseInvalid
ProvisionedRemoteComputeError::ProvisionerWorker(_) => NativeCommandErrorCode::ProvisionerWorkerFailed
ProvisionedRemoteComputeError::ExecuteWorkspaceNotReady => NativeCommandErrorCode::InvalidProvisioningState
ProvisionedRemoteComputeError::ExecuteWorkspaceMissingEndpoint => NativeCommandErrorCode::InvalidProvisioningState
ProvisionedRemoteComputeError::ExecuteWorkspaceNotImplemented { .. } => NativeCommandErrorCode::CommandNotImplemented
ProvisionedRemoteComputeError::DeleteWorkspaceFailed { .. } => NativeCommandErrorCode::ProviderRequestFailed
```

Keep current messages unless a message currently includes renamed runtime words that should be clarified.

- [ ] **Step 8: Run command error tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml native_command_error_serializes_code_and_message
cargo test --manifest-path src-tauri/Cargo.toml export_bindings
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/types/workspace.rs
git commit -m "feat(native): add typed command error codes"
```

---

### Task 8: Regenerate Bindings And Update Frontend References

**Files:**

- Modify: `src/generated/commands.ts`
- Modify: frontend files only if TypeScript build reports references to old generated type names.

- [ ] **Step 1: Regenerate command bindings**

Run:

```bash
bun run codegen:commands
```

Expected: `src/generated/commands.ts` updates with `NativeCommandError.code` and provisioned remote compute runtime type names.

- [ ] **Step 2: Inspect generated diff**

Run:

```bash
git diff -- src/generated/commands.ts
```

Expected changes:

- `NativeCommandError` includes `code`.
- `WorkspaceRuntimeResponse` uses `runtimeType: "provisioned_remote_compute"`.
- DTO names generated from Rust are updated if Specta emits renamed names.

- [ ] **Step 3: Build frontend**

Run:

```bash
bun run build
```

Expected: PASS. If it fails because a frontend file imports old generated type names, update only those imports/usages to the generated names.

- [ ] **Step 4: Run frontend lint**

Run:

```bash
bun run lint
```

Expected: PASS. If lint fails only on pre-existing README formatting, record it in the task notes and do not change README unless this task modified it.

- [ ] **Step 5: Commit**

```bash
git add src/generated/commands.ts src
git commit -m "chore(native): regenerate command bindings"
```

If no frontend files changed besides `src/generated/commands.ts`, use:

```bash
git add src/generated/commands.ts
git commit -m "chore(native): regenerate command bindings"
```

---

### Task 9: Full Native Verification And Final Cleanup

**Files:**

- Modify only files needed to fix verification failures caused by this plan.

- [ ] **Step 1: Run full native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run native format check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS. If it fails, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Then rerun the check.

- [ ] **Step 3: Run native clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Search for stale names**

Run:

```bash
rg "remote_workspace|RemoteWorkspace|WorkspaceRuntime::Remote|runtimeType\":\"remote|remoteResources|remoteProvisioning" src-tauri/src src/generated/commands.ts
```

Expected: no stale old-runtime matches. `RemotePlacement` matches are allowed and should not be renamed by this plan.

- [ ] **Step 5: Check git diff scope**

Run:

```bash
git diff --stat HEAD
git status --short
```

Expected: only native/runtime/frontend generated files changed by the plan are present. No worker, SQLite table schema, persistence migration/versioning, endpoint execution, or unrelated README changes.

- [ ] **Step 6: Final commit if verification fixes changed files**

If Step 2 or Step 3 required fixes, commit them:

```bash
git add src-tauri/src src/generated/commands.ts
git commit -m "chore(native): finalize provisioned compute cleanup"
```

If no files changed, do not create an empty commit.

---

## Self-Review Notes

Spec coverage:

- Runtime rename is covered in Tasks 1 and 2.
- Service decomposition is covered in Tasks 3 through 6.
- Minimal command errors are covered in Task 7.
- Generated command binding update is covered in Task 8.
- Verification is covered in Task 9.
- Out-of-scope items are explicitly excluded in File Structure, Task 8, and Task 9.

Type consistency:

- The plan consistently uses `ProvisionedRemoteCompute` for runtime/domain/module/service concepts.
- The plan keeps `RemotePlacement*` names unchanged because placement is still provider-facing and not the runtime family.
- The plan uses `ProvisionedRemoteComputeError` and `ProvisionedRemoteComputeProvisioningError` after the module/type rename.

Implementation constraint:

- Each task is independently committable and should preserve behavior before moving to the next task.
