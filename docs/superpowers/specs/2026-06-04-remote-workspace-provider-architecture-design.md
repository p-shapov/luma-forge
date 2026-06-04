# Remote Workspace Provider Architecture Design

## Context

LumaForge provisions remote GPU workspaces for ComfyUI workflow execution. The active native backend keeps the workspace domain intentionally small. The current workspace domain lives in `src-tauri/src/domain/workspace.rs`:

- `Workspace` owns the common workspace identity and selected `WorkflowPreset`.
- `WorkspaceRuntime` is a tagged enum.
- `WorkspaceRuntime::Remote(RemoteWorkspace)` is currently the only runtime variant.
- `RemoteWorkspace` stores placement, provisioning state, and provider resource snapshots.

This design keeps that shape. It adds a source-level remote GPU provider boundary and service-level workspace operation skeletons without adding a concrete provider implementation.

The repository checkout used for this design does not contain the `spec/`, `openspec/`, or `docs/` flow files referenced by root `AGENTS.md`. The user-provided flow text is the authoritative product input for this design.

## Goals

- Model only `remote_workspace` as the active workspace runtime.
- Keep `WorkspaceRuntime::Remote(RemoteWorkspace)` as the thin common abstraction.
- Define service-level workspace operations: `setup`, `observe`, `provision`, `execute`, and `delete`.
- Define Rust trait boundaries for future remote provider adapters.
- Define static source-level provider registration.
- Add compile-time Rust skeletons and focused tests for the boundary.

## Non-Goals

- No concrete RunPod implementation.
- No dynamic plugin loading.
- No external provider sidecar process.
- No local workspace runtime.
- No running-hub workspace runtime.
- No frontend workflow changes.
- No Tauri command contract changes.
- No provider SDK integration.
- No real persistence or secure-storage implementation.

## Architecture

Add a new native backend module:

```text
src-tauri/src/remote_workspace/
  mod.rs
  service.rs
  provider.rs
  registry.rs
  errors.rs
```

Expose it from `src-tauri/src/lib.rs` with `pub mod remote_workspace;`.

The new module is an application/service boundary. It consumes the existing domain types instead of replacing them with a general workspace trait hierarchy.

`service.rs` owns service-level use case skeletons:

- `setup_workspace`
- `observe_workspace`
- `provision_workspace`
- `execute_workspace`
- `delete_workspace`

`provider.rs` owns resource-oriented provider traits and parameter structs.

`registry.rs` owns static provider registration and provider lookup.

`errors.rs` owns UI-safe provider and workspace service errors.

No `src-tauri/src/providers/runpod/` module is added in this iteration. `GpuCloudProviderId::Runpod` already exists, but registry lookup for `Runpod` returns an explicit missing-provider error until a real adapter is added later.

## Provider Traits

Provider traits are resource-oriented. They do not own the full workspace orchestration.

Use boxed futures in trait methods to keep the traits object-safe without adding a new dependency:

```rust
type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
```

The common provider trait combines the resource traits:

```rust
pub trait RemoteWorkspaceProvider:
    RemoteVolumeProvider
    + RemoteProvisionerProvider
    + RemoteEndpointProvider
    + Send
    + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
```

The volume trait provides:

- `create_volume`
- `delete_volume`
- `observe_volume`

The provisioner trait provides:

- `start_provisioner`
- `terminate_provisioner`
- `observe_provisioner`
- `get_provisioner_status`

The endpoint trait provides:

- `create_endpoint`
- `delete_endpoint`
- `observe_endpoint`

Provider adapters normalize provider SDK/API errors before returning them. They never return raw provider payloads, raw request bodies, tokens, API keys, or credential-bearing URLs.

## Params And Snapshots

Provider params contain only the data needed for provider calls. The skeleton defines narrow structs for each provider operation rather than passing full `Workspace` values to providers.

Common fields include:

- `workspace_id`
- `datacenter_id`
- `gpu_id`
- `volume_id`
- `endpoint_id`
- `provisioner_id`
- `size_bytes`
- `provisioner_image_ref`
- `endpoint_image_ref`
- `mount_path`

Snapshots use the existing domain snapshot types where possible:

- `RemoteVolumeSnapshot`
- `RemoteProvisionerSnapshot`
- `RemoteEndpointSnapshot`
- `RemoteProvisionerStatus`

Those snapshots remain UI-safe. They must not include provider API keys, bearer tokens, Hugging Face API keys, raw request bodies, raw response bodies, SDK debug output, or credential-bearing URLs.

## Registry

`RemoteWorkspaceProviderRegistry` stores source-level provider adapters:

```rust
pub struct RemoteWorkspaceProviderRegistry {
    providers: Vec<Box<dyn RemoteWorkspaceProvider>>,
}
```

It exposes:

- `new(providers: Vec<Box<dyn RemoteWorkspaceProvider>>) -> Self`
- `empty() -> Self`
- `for_provider(provider_id: GpuCloudProviderId) -> Result<&dyn RemoteWorkspaceProvider, RemoteWorkspaceError>`

Lookup is exact by `GpuCloudProviderId`. Missing providers return `RemoteWorkspaceError::MissingProvider { provider_id }`.

The registry never falls back to a different provider.

Future contributor flow:

1. Add provider id to `GpuCloudProviderId`.
2. Implement the provider adapter traits.
3. Register the adapter in `RemoteWorkspaceProviderRegistry`.
4. Add registry selection tests.
5. Add provider contract tests for the adapter.

## Workspace Operations

`RemoteWorkspaceService` holds the common operation skeletons. The service depends on the provider registry and narrow collaborator traits only where needed for tests.

### setup_workspace

Creates a local Draft remote workspace only.

The skeleton builds a `Workspace` with:

- selected `WorkflowPreset`
- selected `RemotePlacementPlan`
- `WorkspaceRuntime::Remote`
- `RemoteProvisioningStatus::NotStarted`
- `percent: None`
- empty `RemoteWorkspaceResources`

It does not call provider APIs, create remote resources, validate remote existence, read secrets, or expose secrets.

### observe_workspace

Performs provider resource discovery for a remote workspace id. This is a preflight conflict check, not status polling.

The skeleton:

1. Accepts a `Workspace`; repository loading is deferred until the persistence layer exists.
2. Requires `WorkspaceRuntime::Remote`.
3. Selects the provider by `remote_placement.gpu_cloud_provider_id`.
4. Calls provider observe methods in deterministic order: volume, provisioner, endpoint.
5. Returns the first matching conflict as a specific error.
6. Does not mutate durable state.

Conflict errors are:

- `RemoteWorkspaceError::ExistingVolume`
- `RemoteWorkspaceError::ExistingProvisioner`
- `RemoteWorkspaceError::ExistingEndpoint`

### provision_workspace

Defines the common provisioning state machine and advances it. It does not implement real remote provisioning in the skeleton phase.

`provision_workspace` is a step-synchronizing operation. Each invocation reads the current remote workspace state, chooses the next valid action, performs at most one bounded provider or worker step, persists the resulting workspace state when persistence exists, and returns the updated workspace/progress. The frontend or a later scheduler may call it repeatedly until the workspace reaches a terminal state.

It is not a single blocking call that loops until the workspace is ready.

The state machine is represented by the existing `RemoteProvisioningStatus` and `RemoteProvisioningPhase` values on `RemoteWorkspace`:

- `NotStarted`
- `InProgress { phase: CreatingRemoteVolume }`
- `InProgress { phase: StartingRemoteProvisioner }`
- `InProgress { phase: RunningRemoteProvisioner { status } }`
- `InProgress { phase: CleaningUpRemoteProvisioner }`
- `InProgress { phase: CreatingRemoteEndpoint }`
- `InProgress { phase: ValidatingReadiness }`
- `Completed`
- `Failed { phase, code, message }`
- `Cancelling { phase }`

The normal forward path is:

1. Run `observe_workspace` preflight.
2. Create remote volume.
3. Start provisioning runner.
4. Start provisioner worker job.
5. Read provisioner worker status until terminal success or failure.
6. Terminate provisioning runner.
7. Create endpoint.
8. Observe endpoint readiness.
9. Persist Ready workspace state.

Draft means `RemoteProvisioningStatus::NotStarted`. Ready means `RemoteProvisioningStatus::Completed` with a stored endpoint snapshot. Failed means `RemoteProvisioningStatus::Failed`.

On the first `NotStarted` invocation, the service runs `observe_workspace` as a conflict preflight before creating resources. Later invocations do not repeat conflict discovery; they use persisted snapshots and the current provisioning phase to decide the next action.

When a provider or worker returns a non-terminal intermediate status, the service persists the updated phase/status and returns without continuing to the next stage. A later `provision_workspace` call resumes from that persisted state.

When a provider or worker returns a terminal failure, the service persists `RemoteProvisioningStatus::Failed` with UI-safe failure metadata. It preserves known resource snapshots so `delete_workspace` can clean them up later.

Provider implementations expose only resource primitives. They do not duplicate this workflow.

The skeleton returns an explicit `RemoteWorkspaceError::NotImplemented` only where a real provider, worker gateway, repository, or secure-storage collaborator is required. The skeleton still defines the state-machine boundary and tests the initial operation decisions with fake providers.

### execute_workspace

Executes only on a Ready remote workspace in a later implementation step.

The skeleton rejects workspaces that are not ready and verifies that a stored endpoint snapshot exists before returning `RemoteWorkspaceError::NotImplemented`.

It does not automatically provision Draft workspaces and does not hide provisioning failures behind execution errors.

### delete_workspace

Deletes a workspace and its known remote resources.

Future deletion order:

1. Endpoint.
2. Provisioning runner.
3. Volume.
4. Per-workspace provisioner token.
5. Local workspace catalog entry.

Provider not-found errors for known resources may be treated as already deleted. Provider cleanup failures prevent local catalog removal.

The skeleton includes dependency-order tests using a fake provider. It does not implement secure storage or real catalog deletion.

## Error Model

All remote workspace errors are represented by one enum, `RemoteWorkspaceError`.

Provider API errors are normalized into:

- `RemoteWorkspaceError::ProviderUnauthorized`
- `RemoteWorkspaceError::ProviderRateLimited`
- `RemoteWorkspaceError::ProviderTimeout`
- `RemoteWorkspaceError::ProviderRequestFailed { message }`

Resource-specific errors are represented as direct `RemoteWorkspaceError` variants:

- `RemoteWorkspaceError::ExistingVolume`
- `RemoteWorkspaceError::NonExistingVolume`
- `RemoteWorkspaceError::ExistingProvisioner`
- `RemoteWorkspaceError::NonExistingProvisioner`
- `RemoteWorkspaceError::ExistingEndpoint`
- `RemoteWorkspaceError::NonExistingEndpoint`

Errors returned from service skeletons are UI-safe. They do not contain secrets, raw provider payloads, stack traces, environment dumps, or SDK debug output.

## Testing Plan

Add focused Rust unit tests under `remote_workspace` modules.

Registry tests:

- lookup returns the provider registered for a provider id.
- missing provider returns explicit missing-provider error.
- lookup does not fall back to another provider.

Setup tests:

- `setup_workspace` returns `WorkspaceRuntime::Remote`.
- `setup_workspace` initializes provisioning as `NotStarted`.
- `setup_workspace` initializes all remote resource snapshots as `None`.
- `setup_workspace` does not call provider APIs.

Observe tests:

- existing volume returns `RemoteWorkspaceError::ExistingVolume`.
- existing provisioner returns `RemoteWorkspaceError::ExistingProvisioner`.
- existing endpoint returns `RemoteWorkspaceError::ExistingEndpoint`.
- observe does not persist discovered snapshots.

Delete tests:

- delete calls provider cleanup in dependency order: endpoint, provisioner, volume.
- provider not-found cleanup results are treated as already deleted.
- provider cleanup failure prevents local catalog removal.

Provisioning state-machine tests:

- first `NotStarted` provision call runs observe preflight before resource creation.
- provisioning advances by bounded steps instead of looping to completion in one call.
- intermediate provider or worker status is persisted as `InProgress`.
- terminal provider or worker failure is persisted as `Failed` with UI-safe metadata.
- known resource snapshots are preserved after provisioning failure for later cleanup.

Error safety tests:

- provider adapters return UI-safe provider messages only.
- provider errors do not expose raw payload fields because raw payload fields are not present in error types.

## Verification

Implementation phases should run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because this design does not change Tauri command contracts, frontend command codegen, build, and lint are not required for the skeleton implementation phase.
