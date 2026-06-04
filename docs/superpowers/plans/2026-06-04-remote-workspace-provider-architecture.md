# Remote Workspace Provider Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add compile-time Rust skeletons and focused tests for the remote workspace operation surface and source-level provider extension boundary.

**Architecture:** Add `src-tauri/src/remote_workspace/` as an application/service module that consumes the existing `domain::workspace` model. Provider adapters expose resource primitives through object-safe boxed-future traits; `RemoteWorkspaceService` owns setup, observe, provision, execute, and delete operation decisions.

**Tech Stack:** Rust 2021, Tauri backend crate, standard-library boxed futures, `serde`, native `cargo test/fmt/clippy`.

---

## File Structure

- Create `src-tauri/src/remote_workspace/mod.rs`: module exports.
- Create `src-tauri/src/remote_workspace/errors.rs`: UI-safe provider, registry, and workspace operation errors.
- Create `src-tauri/src/remote_workspace/provider.rs`: object-safe provider traits and resource parameter structs.
- Create `src-tauri/src/remote_workspace/registry.rs`: static provider registry and lookup tests.
- Create `src-tauri/src/remote_workspace/operation.rs`: `RemoteWorkspaceService`, operation request structs, collaborator traits, state-machine skeletons, and service tests.
- Modify `src-tauri/src/lib.rs`: expose `pub mod remote_workspace;`.

Do not create `src-tauri/src/providers/` or any concrete RunPod adapter in this plan.

## Task 1: Module Shell And Error Types

**Files:**
- Create: `src-tauri/src/remote_workspace/mod.rs`
- Create: `src-tauri/src/remote_workspace/errors.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the module shell**

Create `src-tauri/src/remote_workspace/mod.rs`:

```rust
pub mod errors;
pub mod operation;
pub mod provider;
pub mod registry;
```

Modify `src-tauri/src/lib.rs` near `pub mod domain;`:

```rust
pub mod domain;
pub mod remote_workspace;
```

- [ ] **Step 2: Write UI-safe error types**

Create `src-tauri/src/remote_workspace/errors.rs`:

```rust
use crate::domain::provider::GpuCloudProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderApiError {
    Unauthorized,
    RateLimited,
    Timeout,
    RequestFailed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVolumeError {
    ExistingVolume,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteVolumeError {
    NonExistingVolume,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveVolumeError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartProvisionerError {
    ExistingProvisioner,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminateProvisionerError {
    NonExistingProvisioner,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveProvisionerError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetProvisionerStatusError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateEndpointError {
    ExistingEndpoint,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteEndpointError {
    NonExistingEndpoint,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveEndpointError {
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteWorkspaceProviderRegistryError {
    MissingProvider { provider_id: GpuCloudProviderId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSetupError {
    InvalidRequest { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceObserveError {
    MissingProvider { provider_id: GpuCloudProviderId },
    ExistingVolume,
    ExistingProvisioner,
    ExistingEndpoint,
    ProviderApi(ProviderApiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceProvisionError {
    MissingProvider { provider_id: GpuCloudProviderId },
    ExistingVolume,
    ExistingProvisioner,
    ExistingEndpoint,
    ProviderApi(ProviderApiError),
    InvalidWorkspaceState { message: String },
    NotImplemented { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceExecuteError {
    WorkspaceNotReady,
    MissingEndpoint,
    NotImplemented { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceDeleteError {
    MissingProvider { provider_id: GpuCloudProviderId },
    CleanupFailed { message: String },
}
```

- [ ] **Step 3: Run compiler to verify expected missing modules**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL with unresolved module files for `operation`, `provider`, or `registry`. This confirms the new module is wired.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/remote_workspace/mod.rs src-tauri/src/remote_workspace/errors.rs
git commit -m "feat(remote-workspace): add service error boundary"
```

## Task 2: Provider Traits And Static Registry

**Files:**
- Create: `src-tauri/src/remote_workspace/provider.rs`
- Create: `src-tauri/src/remote_workspace/registry.rs`

- [ ] **Step 1: Write provider trait skeletons**

Create `src-tauri/src/remote_workspace/provider.rs`:

```rust
use std::{future::Future, pin::Pin};

use crate::domain::{
    provider::GpuCloudProviderId,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
        RemoteVolumeSnapshot,
    },
};

use super::errors::{
    CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
    GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError, ObserveVolumeError,
    StartProvisionerError, TerminateProvisionerError,
};

pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateVolumeParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub size_bytes: u64,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteVolumeParams {
    pub workspace_id: String,
    pub volume_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveVolumeParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProvisionerParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub provisioner_image_ref: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminateProvisionerParams {
    pub workspace_id: String,
    pub provisioner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveProvisionerParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProvisionerStatusParams {
    pub workspace_id: String,
    pub provisioner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEndpointParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub endpoint_image_ref: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: Option<String>,
}

pub trait RemoteVolumeProvider {
    fn create_volume<'a>(
        &'a self,
        params: CreateVolumeParams,
    ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>>;

    fn delete_volume<'a>(
        &'a self,
        params: DeleteVolumeParams,
    ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>>;

    fn observe_volume<'a>(
        &'a self,
        params: ObserveVolumeParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>>;
}

pub trait RemoteProvisionerProvider {
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>>;

    fn terminate_provisioner<'a>(
        &'a self,
        params: TerminateProvisionerParams,
    ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>>;

    fn observe_provisioner<'a>(
        &'a self,
        params: ObserveProvisionerParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        params: GetProvisionerStatusParams,
    ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>>;
}

pub trait RemoteEndpointProvider {
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>>;

    fn delete_endpoint<'a>(
        &'a self,
        params: DeleteEndpointParams,
    ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>>;

    fn observe_endpoint<'a>(
        &'a self,
        params: ObserveEndpointParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>>;
}

pub trait RemoteWorkspaceProvider:
    RemoteVolumeProvider + RemoteProvisionerProvider + RemoteEndpointProvider + Send + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
```

- [ ] **Step 2: Write registry and tests**

Create `src-tauri/src/remote_workspace/registry.rs`:

```rust
use crate::domain::provider::GpuCloudProviderId;

use super::{
    errors::RemoteWorkspaceProviderRegistryError, provider::RemoteWorkspaceProvider,
};

pub struct RemoteWorkspaceProviderRegistry {
    providers: Vec<Box<dyn RemoteWorkspaceProvider>>,
}

impl RemoteWorkspaceProviderRegistry {
    pub fn new(providers: Vec<Box<dyn RemoteWorkspaceProvider>>) -> Self {
        Self { providers }
    }

    pub fn empty() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn for_provider(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<&dyn RemoteWorkspaceProvider, RemoteWorkspaceProviderRegistryError> {
        self.providers
            .iter()
            .find(|provider| provider.provider_id() == provider_id)
            .map(|provider| provider.as_ref())
            .ok_or(RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id })
    }
}
```

Add this test module to the bottom of `registry.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::domain::{
        provider::GpuCloudProviderId,
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteVolumeSnapshot,
        },
    };

    use super::*;
    use crate::remote_workspace::{
        errors::{
            CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
            GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError,
            ObserveVolumeError, StartProvisionerError, TerminateProvisionerError,
        },
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, ObserveEndpointParams, ObserveProvisionerParams,
            ObserveVolumeParams, ProviderFuture, RemoteEndpointProvider,
            RemoteProvisionerProvider, RemoteVolumeProvider, StartProvisionerParams,
            TerminateProvisionerParams,
        },
    };

    struct FakeProvider {
        provider_id: GpuCloudProviderId,
    }

    impl RemoteVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            _params: CreateVolumeParams,
        ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>> {
            Box::pin(async { Ok(RemoteVolumeSnapshot { id: "volume".to_string() }) })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_volume<'a>(
            &'a self,
            _params: ObserveVolumeParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>> {
            Box::pin(async { Ok(None) })
        }
    }

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            _params: StartProvisionerParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>> {
            Box::pin(async {
                Ok(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_provisioner<'a>(
            &'a self,
            _params: ObserveProvisionerParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>
        {
            Box::pin(async { Ok(None) })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            _params: GetProvisionerStatusParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>> {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>> {
            Box::pin(async {
                Ok(RemoteEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _params: DeleteEndpointParams,
        ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_endpoint<'a>(
            &'a self,
            _params: ObserveEndpointParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>> {
            Box::pin(async { Ok(None) })
        }
    }

    impl RemoteWorkspaceProvider for FakeProvider {
        fn provider_id(&self) -> GpuCloudProviderId {
            self.provider_id
        }
    }

    #[test]
    fn lookup_returns_registered_provider() {
        let registry = RemoteWorkspaceProviderRegistry::new(vec![Box::new(FakeProvider {
            provider_id: GpuCloudProviderId::Runpod,
        })]);

        let provider = registry
            .for_provider(GpuCloudProviderId::Runpod)
            .expect("registered provider should resolve");

        assert_eq!(provider.provider_id(), GpuCloudProviderId::Runpod);
    }

    #[test]
    fn missing_provider_returns_explicit_error() {
        let registry = RemoteWorkspaceProviderRegistry::empty();

        let error = registry
            .for_provider(GpuCloudProviderId::Runpod)
            .expect_err("missing provider should fail");

        assert_eq!(
            error,
            RemoteWorkspaceProviderRegistryError::MissingProvider {
                provider_id: GpuCloudProviderId::Runpod
            }
        );
    }
}
```

- [ ] **Step 3: Run registry tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace::registry
```

Expected: PASS for both registry tests, or FAIL only because `operation.rs` is still missing.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/remote_workspace/provider.rs src-tauri/src/remote_workspace/registry.rs
git commit -m "feat(remote-workspace): add provider registry boundary"
```

## Task 3: Setup And Observe Service Skeleton

**Files:**
- Create: `src-tauri/src/remote_workspace/operation.rs`

- [ ] **Step 1: Write service setup and observe tests first**

Create `src-tauri/src/remote_workspace/operation.rs` with imports, request structs, and test helpers. Start with this full file; the initial compile may fail until later steps fill all methods:

```rust
use crate::domain::{
    placement::RemotePlacementPlan,
    provider::GpuCloudProviderId,
    workflow_preset::WorkflowPreset,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisioningState, RemoteProvisioningStatus,
        RemoteProvisionerSnapshot, RemoteWorkspace, RemoteWorkspaceResources,
        RemoteVolumeSnapshot, Workspace, WorkspaceRuntime,
    },
};

use super::{
    errors::{
        DeleteEndpointError, DeleteVolumeError, ObserveEndpointError, ObserveProvisionerError,
        ObserveVolumeError, RemoteWorkspaceProviderRegistryError, TerminateProvisionerError,
        WorkspaceDeleteError, WorkspaceExecuteError, WorkspaceObserveError,
        WorkspaceProvisionError, WorkspaceSetupError,
    },
    provider::{
        DeleteEndpointParams, DeleteVolumeParams, ObserveEndpointParams, ObserveProvisionerParams,
        ObserveVolumeParams, TerminateProvisionerParams,
    },
    registry::RemoteWorkspaceProviderRegistry,
};

pub struct SetupWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset: WorkflowPreset,
    pub remote_placement: RemotePlacementPlan,
}

pub struct RemoteWorkspaceService {
    provider_registry: RemoteWorkspaceProviderRegistry,
}

impl RemoteWorkspaceService {
    pub fn new(provider_registry: RemoteWorkspaceProviderRegistry) -> Self {
        Self { provider_registry }
    }

    pub fn setup_workspace(
        &self,
        request: SetupWorkspaceRequest,
    ) -> Result<Workspace, WorkspaceSetupError> {
        if request.workspace_id.trim().is_empty() {
            return Err(WorkspaceSetupError::InvalidRequest {
                message: "workspace id is required".to_string(),
            });
        }

        Ok(Workspace {
            id: request.workspace_id,
            workflow_preset: request.workflow_preset,
            runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
                remote_placement: request.remote_placement,
                remote_provisioning: RemoteProvisioningState {
                    status: RemoteProvisioningStatus::NotStarted,
                    percent: None,
                },
                remote_resources: RemoteWorkspaceResources {
                    remote_volume: None,
                    remote_provisioner: None,
                    remote_endpoint: None,
                },
            }),
        })
    }

    pub async fn observe_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<(), WorkspaceObserveError> {
        let remote = remote_workspace(workspace);
        let provider_id = remote.remote_placement.gpu_cloud_provider_id;
        let provider = self
            .provider_registry
            .for_provider(provider_id)
            .map_err(workspace_observe_registry_error)?;

        if provider
            .observe_volume(ObserveVolumeParams {
                workspace_id: workspace.id.clone(),
            })
            .await
            .map_err(workspace_observe_volume_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingVolume);
        }

        if provider
            .observe_provisioner(ObserveProvisionerParams {
                workspace_id: workspace.id.clone(),
            })
            .await
            .map_err(workspace_observe_provisioner_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingProvisioner);
        }

        if provider
            .observe_endpoint(ObserveEndpointParams {
                workspace_id: workspace.id.clone(),
                endpoint_id: remote
                    .remote_resources
                    .remote_endpoint
                    .as_ref()
                    .map(|endpoint| endpoint.id.clone()),
            })
            .await
            .map_err(workspace_observe_endpoint_error)?
            .is_some()
        {
            return Err(WorkspaceObserveError::ExistingEndpoint);
        }

        Ok(())
    }
}

fn remote_workspace(workspace: &Workspace) -> &RemoteWorkspace {
    match &workspace.runtime {
        WorkspaceRuntime::Remote(remote) => remote,
    }
}

fn workspace_observe_registry_error(
    error: RemoteWorkspaceProviderRegistryError,
) -> WorkspaceObserveError {
    match error {
        RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id } => {
            WorkspaceObserveError::MissingProvider { provider_id }
        }
    }
}

fn workspace_observe_volume_error(error: ObserveVolumeError) -> WorkspaceObserveError {
    match error {
        ObserveVolumeError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}

fn workspace_observe_provisioner_error(error: ObserveProvisionerError) -> WorkspaceObserveError {
    match error {
        ObserveProvisionerError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}

fn workspace_observe_endpoint_error(error: ObserveEndpointError) -> WorkspaceObserveError {
    match error {
        ObserveEndpointError::ProviderApi(error) => WorkspaceObserveError::ProviderApi(error),
    }
}
```

- [ ] **Step 2: Add setup/observe tests**

Append tests to `operation.rs` using the fake provider pattern from Task 2. Include these test names exactly:

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        placement::{
            Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
            RemotePlacementPlan,
        },
        runtime_contract::RuntimeContractReference,
        workflow_preset::{
            RemoteProviderRuntimeRequirements, RemoteRuntimeRequirements, WorkflowExecutionType,
            WorkflowPreset,
        },
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerStatus, RemoteWorkspaceResources,
            RemoteVolumeSnapshot,
        },
    };

    use super::*;
    use crate::remote_workspace::{
        errors::{
            CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
            GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError,
            ObserveVolumeError, StartProvisionerError, TerminateProvisionerError,
        },
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, ObserveEndpointParams, ObserveProvisionerParams,
            ObserveVolumeParams, ProviderFuture, RemoteEndpointProvider,
            RemoteProvisionerProvider, RemoteVolumeProvider, RemoteWorkspaceProvider,
            StartProvisionerParams, TerminateProvisionerParams,
        },
    };

    #[derive(Default)]
    struct ProviderState {
        calls: Vec<&'static str>,
        volume: Option<RemoteVolumeSnapshot>,
        provisioner: Option<RemoteProvisionerSnapshot>,
        endpoint: Option<RemoteEndpointSnapshot>,
    }

    struct FakeProvider {
        state: Arc<Mutex<ProviderState>>,
    }

    impl FakeProvider {
        fn new(state: Arc<Mutex<ProviderState>>) -> Self {
            Self { state }
        }
    }

    impl RemoteVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            _params: CreateVolumeParams,
        ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>> {
            Box::pin(async { Ok(RemoteVolumeSnapshot { id: "volume".to_string() }) })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_volume<'a>(
            &'a self,
            _params: ObserveVolumeParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_volume");
                Ok(state.volume.clone())
            })
        }
    }

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            _params: StartProvisionerParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>> {
            Box::pin(async {
                Ok(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_provisioner<'a>(
            &'a self,
            _params: ObserveProvisionerParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_provisioner");
                Ok(state.provisioner.clone())
            })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            _params: GetProvisionerStatusParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>> {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>> {
            Box::pin(async {
                Ok(RemoteEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _params: DeleteEndpointParams,
        ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_endpoint<'a>(
            &'a self,
            _params: ObserveEndpointParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("observe_endpoint");
                Ok(state.endpoint.clone())
            })
        }
    }

    impl RemoteWorkspaceProvider for FakeProvider {
        fn provider_id(&self) -> GpuCloudProviderId {
            GpuCloudProviderId::Runpod
        }
    }

    fn service_with_state(state: Arc<Mutex<ProviderState>>) -> RemoteWorkspaceService {
        RemoteWorkspaceService::new(RemoteWorkspaceProviderRegistry::new(vec![Box::new(
            FakeProvider::new(state),
        )]))
    }

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "preset".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: false,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 1,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "endpoint".to_string(),
                        version: "1".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner".to_string(),
                        version: "1".to_string(),
                    },
                }],
            },
            required_model_assets: vec![],
        }
    }

    fn placement_plan() -> RemotePlacementPlan {
        RemotePlacementPlan {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            datacenter_id: "dc".to_string(),
            gpu_id: "gpu".to_string(),
            remote_volume_size_bytes: 1,
            remote_capabilities: RemotePlacementCapabilities {
                remote_endpoint_keep_alive: Capability::Supported(RemoteEndpointKeepAliveLimits {
                    default_seconds: 60,
                    min_seconds: 30,
                    max_seconds: 120,
                }),
            },
        }
    }

    fn draft_workspace(service: &RemoteWorkspaceService) -> Workspace {
        service
            .setup_workspace(SetupWorkspaceRequest {
                workspace_id: "workspace".to_string(),
                workflow_preset: workflow_preset(),
                remote_placement: placement_plan(),
            })
            .expect("workspace setup should succeed")
    }

    #[test]
    fn setup_workspace_returns_remote_runtime_with_not_started_state() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(Arc::clone(&state));

        let workspace = draft_workspace(&service);

        let WorkspaceRuntime::Remote(remote) = workspace.runtime;
        assert_eq!(remote.remote_provisioning.status, RemoteProvisioningStatus::NotStarted);
        assert_eq!(remote.remote_provisioning.percent, None);
        assert_eq!(
            remote.remote_resources,
            RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            }
        );
        assert!(state.lock().expect("state lock should succeed").calls.is_empty());
    }
}
```

- [ ] **Step 3: Run setup test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml setup_workspace_returns_remote_runtime_with_not_started_state
```

Expected: PASS.

- [ ] **Step 4: Add observe conflict tests**

Append these tests inside the existing `tests` module:

```rust
#[test]
fn observe_workspace_returns_existing_volume_conflict() {
    let state = Arc::new(Mutex::new(ProviderState {
        volume: Some(RemoteVolumeSnapshot { id: "volume".to_string() }),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let workspace = draft_workspace(&service);

    let error = block_on(service.observe_workspace(&workspace))
        .expect_err("existing volume should be a conflict");

    assert_eq!(error, WorkspaceObserveError::ExistingVolume);
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["observe_volume"]
    );
}

#[test]
fn observe_workspace_returns_existing_provisioner_conflict() {
    let state = Arc::new(Mutex::new(ProviderState {
        provisioner: Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        }),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let workspace = draft_workspace(&service);

    let error = block_on(service.observe_workspace(&workspace))
        .expect_err("existing provisioner should be a conflict");

    assert_eq!(error, WorkspaceObserveError::ExistingProvisioner);
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["observe_volume", "observe_provisioner"]
    );
}

#[test]
fn observe_workspace_returns_existing_endpoint_conflict() {
    let state = Arc::new(Mutex::new(ProviderState {
        endpoint: Some(RemoteEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        }),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let workspace = draft_workspace(&service);

    let error = block_on(service.observe_workspace(&workspace))
        .expect_err("existing endpoint should be a conflict");

    assert_eq!(error, WorkspaceObserveError::ExistingEndpoint);
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["observe_volume", "observe_provisioner", "observe_endpoint"]
    );
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::{
        pin::Pin,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };

    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => {}
        }
    }
}
```

- [ ] **Step 5: Run observe tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml observe_workspace
```

Expected: PASS for all observe tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/remote_workspace/operation.rs
git commit -m "feat(remote-workspace): add setup and observe skeleton"
```

## Task 4: Provision State-Machine Skeleton

**Files:**
- Modify: `src-tauri/src/remote_workspace/operation.rs`

- [ ] **Step 1: Add provisioning method skeleton**

Add these imports to `operation.rs`:

```rust
use crate::domain::workspace::{RemoteProvisioningPhase, RemoteProvisionerStatus};
use super::errors::{CreateVolumeError, StartProvisionerError};
use super::provider::{CreateVolumeParams, StartProvisionerParams};
```

Add this method inside `impl RemoteWorkspaceService`:

```rust
pub async fn provision_workspace(
    &self,
    workspace: &Workspace,
) -> Result<Workspace, WorkspaceProvisionError> {
    let mut updated = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut updated.runtime;
    let provider_id = remote.remote_placement.gpu_cloud_provider_id;
    let provider = self
        .provider_registry
        .for_provider(provider_id)
        .map_err(workspace_provision_registry_error)?;

    match &remote.remote_provisioning.status {
        RemoteProvisioningStatus::NotStarted => {
            self.observe_workspace(workspace)
                .await
                .map_err(workspace_provision_observe_error)?;
            let volume = provider
                .create_volume(CreateVolumeParams {
                    workspace_id: updated.id.clone(),
                    datacenter_id: remote.remote_placement.datacenter_id.clone(),
                    gpu_id: remote.remote_placement.gpu_id.clone(),
                    size_bytes: remote.remote_placement.remote_volume_size_bytes,
                    mount_path: "/workspace".to_string(),
                })
                .await
                .map_err(workspace_provision_create_volume_error)?;
            remote.remote_resources.remote_volume = Some(volume);
            remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
            };
            remote.remote_provisioning.percent = Some(25);
            Ok(updated)
        }
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        } => {
            let volume_id = remote
                .remote_resources
                .remote_volume
                .as_ref()
                .ok_or_else(|| WorkspaceProvisionError::InvalidWorkspaceState {
                    message: "remote volume snapshot is required before provisioner start"
                        .to_string(),
                })?
                .id
                .clone();
            let provisioner = provider
                .start_provisioner(StartProvisionerParams {
                    workspace_id: updated.id.clone(),
                    datacenter_id: remote.remote_placement.datacenter_id.clone(),
                    gpu_id: remote.remote_placement.gpu_id.clone(),
                    volume_id,
                    provisioner_image_ref: "unresolved-provisioner-image".to_string(),
                    mount_path: "/workspace".to_string(),
                })
                .await
                .map_err(workspace_provision_start_provisioner_error)?;
            remote.remote_resources.remote_provisioner = Some(provisioner);
            remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Pending,
                },
            };
            remote.remote_provisioning.percent = Some(50);
            Ok(updated)
        }
        RemoteProvisioningStatus::Completed => Ok(updated),
        RemoteProvisioningStatus::Failed { .. } => Err(WorkspaceProvisionError::InvalidWorkspaceState {
            message: "failed workspace must be deleted or reset before provisioning can continue"
                .to_string(),
        }),
        _ => Err(WorkspaceProvisionError::NotImplemented {
            message: "provisioning step is not implemented in this skeleton".to_string(),
        }),
    }
}
```

Add mapping helpers below the existing observe helpers:

```rust
fn workspace_provision_registry_error(
    error: RemoteWorkspaceProviderRegistryError,
) -> WorkspaceProvisionError {
    match error {
        RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id } => {
            WorkspaceProvisionError::MissingProvider { provider_id }
        }
    }
}

fn workspace_provision_observe_error(error: WorkspaceObserveError) -> WorkspaceProvisionError {
    match error {
        WorkspaceObserveError::MissingProvider { provider_id } => {
            WorkspaceProvisionError::MissingProvider { provider_id }
        }
        WorkspaceObserveError::ExistingVolume => WorkspaceProvisionError::ExistingVolume,
        WorkspaceObserveError::ExistingProvisioner => WorkspaceProvisionError::ExistingProvisioner,
        WorkspaceObserveError::ExistingEndpoint => WorkspaceProvisionError::ExistingEndpoint,
        WorkspaceObserveError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
}

fn workspace_provision_create_volume_error(error: CreateVolumeError) -> WorkspaceProvisionError {
    match error {
        CreateVolumeError::ExistingVolume => WorkspaceProvisionError::ExistingVolume,
        CreateVolumeError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
}

fn workspace_provision_start_provisioner_error(
    error: StartProvisionerError,
) -> WorkspaceProvisionError {
    match error {
        StartProvisionerError::ExistingProvisioner => WorkspaceProvisionError::ExistingProvisioner,
        StartProvisionerError::ProviderApi(error) => WorkspaceProvisionError::ProviderApi(error),
    }
}
```

- [ ] **Step 2: Update fake provider call logging**

In the `FakeProvider` test implementation, modify `create_volume` to log:

```rust
let mut state = self.state.lock().expect("state lock should succeed");
state.calls.push("create_volume");
Ok(RemoteVolumeSnapshot { id: "volume".to_string() })
```

Modify `start_provisioner` to log:

```rust
let mut state = self.state.lock().expect("state lock should succeed");
state.calls.push("start_provisioner");
Ok(RemoteProvisionerSnapshot {
    id: "provisioner".to_string(),
    status_url: "https://status.example".to_string(),
})
```

- [ ] **Step 3: Add provisioning state-machine tests**

Append these tests:

```rust
#[test]
fn provision_workspace_not_started_runs_preflight_then_creates_volume_only() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let workspace = draft_workspace(&service);

    let updated = block_on(service.provision_workspace(&workspace))
        .expect("first provision step should succeed");

    let WorkspaceRuntime::Remote(remote) = updated.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
        }
    );
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot { id: "volume".to_string() })
    );
    assert_eq!(remote.remote_resources.remote_provisioner, None);
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec![
            "observe_volume",
            "observe_provisioner",
            "observe_endpoint",
            "create_volume"
        ]
    );
}

#[test]
fn provision_workspace_starting_provisioner_advances_one_step() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
    };

    let updated = block_on(service.provision_workspace(&workspace))
        .expect("provisioner step should succeed");

    let WorkspaceRuntime::Remote(remote) = updated.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Pending,
            },
        }
    );
    assert_eq!(
        remote.remote_resources.remote_provisioner,
        Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        })
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["start_provisioner"]
    );
}
```

- [ ] **Step 4: Run provisioning tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/remote_workspace/operation.rs
git commit -m "feat(remote-workspace): add provisioning state skeleton"
```

## Task 5: Execute And Delete Skeletons

**Files:**
- Modify: `src-tauri/src/remote_workspace/operation.rs`

- [ ] **Step 1: Add execute and delete methods**

Add these methods inside `impl RemoteWorkspaceService`:

```rust
pub fn execute_workspace(&self, workspace: &Workspace) -> Result<(), WorkspaceExecuteError> {
    let remote = remote_workspace(workspace);
    if remote.remote_provisioning.status != RemoteProvisioningStatus::Completed {
        return Err(WorkspaceExecuteError::WorkspaceNotReady);
    }
    if remote.remote_resources.remote_endpoint.is_none() {
        return Err(WorkspaceExecuteError::MissingEndpoint);
    }
    Err(WorkspaceExecuteError::NotImplemented {
        message: "endpoint worker execution is not implemented in this skeleton".to_string(),
    })
}

pub async fn delete_workspace(&self, workspace: &Workspace) -> Result<(), WorkspaceDeleteError> {
    let remote = remote_workspace(workspace);
    let provider_id = remote.remote_placement.gpu_cloud_provider_id;
    let provider = self
        .provider_registry
        .for_provider(provider_id)
        .map_err(workspace_delete_registry_error)?;

    if let Some(endpoint) = &remote.remote_resources.remote_endpoint {
        match provider
            .delete_endpoint(DeleteEndpointParams {
                workspace_id: workspace.id.clone(),
                endpoint_id: endpoint.id.clone(),
            })
            .await
        {
            Ok(()) | Err(DeleteEndpointError::NonExistingEndpoint) => {}
            Err(DeleteEndpointError::ProviderApi(_)) => {
                return Err(WorkspaceDeleteError::CleanupFailed {
                    message: "endpoint cleanup failed".to_string(),
                });
            }
        }
    }

    if let Some(provisioner) = &remote.remote_resources.remote_provisioner {
        match provider
            .terminate_provisioner(TerminateProvisionerParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: provisioner.id.clone(),
            })
            .await
        {
            Ok(()) | Err(TerminateProvisionerError::NonExistingProvisioner) => {}
            Err(TerminateProvisionerError::ProviderApi(_)) => {
                return Err(WorkspaceDeleteError::CleanupFailed {
                    message: "provisioner cleanup failed".to_string(),
                });
            }
        }
    }

    if let Some(volume) = &remote.remote_resources.remote_volume {
        match provider
            .delete_volume(DeleteVolumeParams {
                workspace_id: workspace.id.clone(),
                volume_id: volume.id.clone(),
            })
            .await
        {
            Ok(()) | Err(DeleteVolumeError::NonExistingVolume) => {}
            Err(DeleteVolumeError::ProviderApi(_)) => {
                return Err(WorkspaceDeleteError::CleanupFailed {
                    message: "volume cleanup failed".to_string(),
                });
            }
        }
    }

    Ok(())
}
```

Add delete mapping helpers:

```rust
fn workspace_delete_registry_error(
    error: RemoteWorkspaceProviderRegistryError,
) -> WorkspaceDeleteError {
    match error {
        RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id } => {
            WorkspaceDeleteError::MissingProvider { provider_id }
        }
    }
}

```

- [ ] **Step 2: Update fake provider delete logging**

In tests, modify fake delete methods to push calls:

```rust
state.calls.push("delete_endpoint");
state.calls.push("terminate_provisioner");
state.calls.push("delete_volume");
```

- [ ] **Step 3: Add execute/delete tests**

Append:

```rust
#[test]
fn execute_workspace_rejects_non_ready_workspace() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(state);
    let workspace = draft_workspace(&service);

    let error = service
        .execute_workspace(&workspace)
        .expect_err("draft workspace should not execute");

    assert_eq!(error, WorkspaceExecuteError::WorkspaceNotReady);
}

#[test]
fn delete_workspace_cleans_resources_in_dependency_order() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_endpoint = Some(RemoteEndpointSnapshot {
        id: "endpoint".to_string(),
        url: "https://endpoint.example".to_string(),
    });
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });

    block_on(service.delete_workspace(&workspace)).expect("delete should succeed");

    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_endpoint", "terminate_provisioner", "delete_volume"]
    );
}
```

- [ ] **Step 4: Run execute/delete tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml execute_workspace delete_workspace
```

Expected: If Cargo rejects multiple test filters, run these separately:

```bash
cargo test --manifest-path src-tauri/Cargo.toml execute_workspace
cargo test --manifest-path src-tauri/Cargo.toml delete_workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/remote_workspace/operation.rs
git commit -m "feat(remote-workspace): add execute and delete skeletons"
```

## Task 6: Final Verification And Cleanup

**Files:**
- Modify only files changed in previous tasks if compiler, formatter, or clippy identifies issues.

- [ ] **Step 1: Run full Rust tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run formatter check**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS. If it fails, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Then rerun the check.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Inspect git diff**

```bash
git status --short
git diff --stat
```

Expected changed files:

```text
src-tauri/src/lib.rs
src-tauri/src/remote_workspace/mod.rs
src-tauri/src/remote_workspace/errors.rs
src-tauri/src/remote_workspace/provider.rs
src-tauri/src/remote_workspace/registry.rs
src-tauri/src/remote_workspace/operation.rs
```

- [ ] **Step 5: Final commit if verification changed formatting**

If Step 2 or Step 3 required changes after Task 5, commit them:

```bash
git add src-tauri/src/lib.rs src-tauri/src/remote_workspace
git commit -m "chore(remote-workspace): verify provider skeleton"
```

If no files changed, do not create an empty commit.

## Self-Review

- Spec coverage: covered module layout, operation skeletons, provider traits, registry behavior, setup, observe conflicts, bounded provision steps, execute readiness rejection, delete order, UI-safe error shapes, and native verification commands.
- Red-flag language scan: clean; the plan gives concrete file paths, snippets, commands, and expected outcomes.
- Type consistency: names match current domain types in `src-tauri/src/domain/workspace.rs`, `placement.rs`, `provider.rs`, `workflow_preset.rs`, and `runtime_contract.rs`.
