# Placement Options Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add remote provider placement option retrieval to the existing `RemoteWorkspaceService` boundary.

**Architecture:** `RemoteWorkspaceService` resolves a provider from `RemoteWorkspaceProviderRegistry` and delegates placement option retrieval to that provider. Provider adapters own credential access internally; the service receives only UI-safe `RemotePlacementOptions` or `RemoteWorkspaceError`.

**Tech Stack:** Rust 2021, Tauri native backend, existing `AppFuture` async trait pattern, standard Rust unit tests.

---

## File Structure

- Modify `src-tauri/src/remote_workspace/provider.rs`: add `RemotePlacementOptionsProvider` and include it in the `RemoteWorkspaceProvider` supertrait.
- Modify `src-tauri/src/remote_workspace/service.rs`: add `RemoteWorkspaceService::get_provider_placement_options` plus focused service tests.
- Modify `src-tauri/src/remote_workspace/registry.rs`: update the registry test fake provider to satisfy the new provider trait bound.
- Do not modify `src-tauri/src/domain/placement.rs` in this plan. Use the current `RemotePlacementOptions` shape:

```rust
pub struct RemotePlacementOptions {
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<RemoteDatacenterPlacementOption>,
}
```

---

### Task 1: Add Failing Service Tests

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Add placement option test imports**

In `src-tauri/src/remote_workspace/service.rs`, inside `#[cfg(test)] mod tests`, change the placement import from:

```rust
placement::{RemoteEndpointKeepAliveLimits, RemotePlacementPlan},
```

to:

```rust
placement::{
    RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits, RemoteGpuPlacementOption,
    RemotePlacementOptions, RemotePlacementPlan,
},
```

In the test provider import list, change:

```rust
GetProvisionerStatusParams, RemoteEndpointProvider, RemoteProvisionerProvider,
RemoteVolumeProvider, RemoteWorkspaceProvider, StartProvisionerParams,
TerminateProvisionerParams,
```

to:

```rust
GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
RemoteProvisionerProvider, RemoteVolumeProvider, RemoteWorkspaceProvider,
StartProvisionerParams, TerminateProvisionerParams,
```

- [ ] **Step 2: Add fake provider placement state**

In `ProviderState`, add this field after `calls`:

```rust
placement_options_result: Option<Result<RemotePlacementOptions, RemoteWorkspaceError>>,
```

- [ ] **Step 3: Add a placement option fixture**

Add this helper near the existing `provider_request_failed` helper:

```rust
fn placement_options() -> RemotePlacementOptions {
    RemotePlacementOptions {
        max_persistent_storage_volume_size_bytes: Some(10),
        datacenters: vec![RemoteDatacenterPlacementOption {
            id: "dc".to_string(),
            name: "Datacenter".to_string(),
            gpu_options: vec![RemoteGpuPlacementOption {
                id: "gpu".to_string(),
                name: "GPU".to_string(),
                vram_bytes: 24,
                availability_score: 90,
            }],
        }],
    }
}
```

- [ ] **Step 4: Add a fake placement options provider implementation**

Add this impl before `impl RemoteVolumeProvider for FakeProvider`:

```rust
impl RemotePlacementOptionsProvider for FakeProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("get_provider_placement_options");

            state
                .placement_options_result
                .clone()
                .unwrap_or_else(|| Ok(placement_options()))
        })
    }
}
```

- [ ] **Step 5: Add service tests**

Add these tests near the other `RemoteWorkspaceService` setup/provision tests:

```rust
#[test]
fn get_provider_placement_options_returns_selected_provider_options() {
    let state = Arc::new(Mutex::new(ProviderState {
        placement_options_result: Some(Ok(placement_options())),
        ..ProviderState::default()
    }));
    let service = service_with_state(state.clone());

    let options = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
        .expect("placement options should be returned");

    assert_eq!(options, placement_options());
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["get_provider_placement_options"]
    );
}

#[test]
fn get_provider_placement_options_returns_provider_unavailable() {
    let service = RemoteWorkspaceService::new(
        RemoteWorkspaceProviderRegistry::empty(),
        WorkflowCatalogService::new(),
    );

    let error = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
        .expect_err("missing provider should fail");

    assert_eq!(
        error,
        RemoteWorkspaceError::ProviderUnavailable {
            provider_id: GpuCloudProviderId::Runpod
        }
    );
}
```

- [ ] **Step 6: Run service tests and verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::service::tests::get_provider_placement_options
```

Expected: FAIL to compile because `RemotePlacementOptionsProvider` and `RemoteWorkspaceService::get_provider_placement_options` do not exist yet.

---

### Task 2: Add Provider Capability And Service Method

**Files:**
- Modify: `src-tauri/src/remote_workspace/provider.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Import placement options in provider traits**

In `src-tauri/src/remote_workspace/provider.rs`, change:

```rust
placement::RemoteEndpointKeepAliveLimits,
```

to:

```rust
placement::{RemoteEndpointKeepAliveLimits, RemotePlacementOptions},
```

- [ ] **Step 2: Add the placement options provider trait**

In `src-tauri/src/remote_workspace/provider.rs`, add this trait before `pub trait RemoteVolumeProvider`:

```rust
pub trait RemotePlacementOptionsProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>>;
}
```

- [ ] **Step 3: Include the capability in `RemoteWorkspaceProvider`**

In `src-tauri/src/remote_workspace/provider.rs`, replace:

```rust
pub trait RemoteWorkspaceProvider:
    RemoteVolumeProvider + RemoteProvisionerProvider + RemoteEndpointProvider + Send + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
```

with:

```rust
pub trait RemoteWorkspaceProvider:
    RemotePlacementOptionsProvider
    + RemoteVolumeProvider
    + RemoteProvisionerProvider
    + RemoteEndpointProvider
    + Send
    + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
```

- [ ] **Step 4: Import service method types**

In `src-tauri/src/remote_workspace/service.rs`, change the domain import from:

```rust
placement::RemotePlacementPlan,
runtime_contract::RuntimeContractReference,
workflow_preset::WorkflowPreset,
```

to:

```rust
placement::{RemotePlacementOptions, RemotePlacementPlan},
provider::GpuCloudProviderId,
runtime_contract::RuntimeContractReference,
workflow_preset::WorkflowPreset,
```

- [ ] **Step 5: Add the service method**

In `impl RemoteWorkspaceService`, add this method after `setup_workspace` and before `provision_workspace`:

```rust
pub async fn get_provider_placement_options(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<RemotePlacementOptions, RemoteWorkspaceError> {
    let provider = self.provider_registry.for_provider(provider_id)?;

    provider.get_provider_placement_options().await
}
```

- [ ] **Step 6: Run service tests and verify the remaining compile failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::service::tests::get_provider_placement_options
```

Expected: FAIL to compile because the registry test fake provider does not implement `RemotePlacementOptionsProvider` yet. Continue to Task 3.

---

### Task 3: Update Registry Test Provider

**Files:**
- Modify: `src-tauri/src/remote_workspace/registry.rs`

- [ ] **Step 1: Add placement imports**

In `src-tauri/src/remote_workspace/registry.rs`, inside `#[cfg(test)] mod tests`, change the domain import from:

```rust
use crate::domain::{
    provider::GpuCloudProviderId,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
        RemoteVolumeSnapshot,
    },
};
```

to:

```rust
use crate::domain::{
    placement::{
        RemoteDatacenterPlacementOption, RemoteGpuPlacementOption, RemotePlacementOptions,
    },
    provider::GpuCloudProviderId,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
        RemoteVolumeSnapshot,
    },
};
```

In the test provider import list, change:

```rust
GetProvisionerStatusParams, RemoteEndpointProvider, RemoteProvisionerProvider,
RemoteVolumeProvider, StartProvisionerParams, TerminateProvisionerParams,
```

to:

```rust
GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
RemoteProvisionerProvider, RemoteVolumeProvider, StartProvisionerParams,
TerminateProvisionerParams,
```

- [ ] **Step 2: Implement placement options for the registry fake provider**

Add this impl before `impl RemoteVolumeProvider for FakeProvider`:

```rust
impl RemotePlacementOptionsProvider for FakeProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
        Box::pin(async {
            Ok(RemotePlacementOptions {
                max_persistent_storage_volume_size_bytes: Some(10),
                datacenters: vec![RemoteDatacenterPlacementOption {
                    id: "dc".to_string(),
                    name: "Datacenter".to_string(),
                    gpu_options: vec![RemoteGpuPlacementOption {
                        id: "gpu".to_string(),
                        name: "GPU".to_string(),
                        vram_bytes: 24,
                        availability_score: 90,
                    }],
                }],
            })
        })
    }
}
```

- [ ] **Step 3: Run service and registry tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::service::tests::get_provider_placement_options
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::registry::tests
```

Expected: PASS for both commands.

- [ ] **Step 4: Commit placement option service boundary**

Run:

```bash
git add src-tauri/src/remote_workspace/provider.rs src-tauri/src/remote_workspace/service.rs src-tauri/src/remote_workspace/registry.rs
git commit -m "feat(remote-workspace): add placement options service method"
```

---

### Task 4: Full Native Verification

**Files:**
- Verify: `src-tauri/src/remote_workspace/provider.rs`
- Verify: `src-tauri/src/remote_workspace/service.rs`
- Verify: `src-tauri/src/remote_workspace/registry.rs`

- [ ] **Step 1: Run all native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run formatting check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Inspect final diff**

Run:

```bash
git status --short
git diff --stat HEAD
```

Expected: only the planned `remote_workspace` files are changed, unless earlier tasks committed them.

---

## Out Of Scope

- Creating a concrete Runpod provider adapter.
- Injecting `secrets_storage` into a concrete provider.
- Wiring Tauri command app state.
- Regenerating frontend command bindings.

Those belong in the next implementation slice after the backend service/provider capability exists.
