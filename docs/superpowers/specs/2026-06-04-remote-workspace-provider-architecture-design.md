# Remote Workspace Provider Architecture Design

## Context

LumaForge is refactoring its active Tauri backend around a small domain model. The current workspace domain lives in `src-tauri/src/domain/workspace.rs`:

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
  operation.rs
  provider.rs
  registry.rs
  errors.rs
```

Expose it from `src-tauri/src/lib.rs` with `pub mod remote_workspace;`.

The new module is an application/service boundary. It consumes the existing domain types instead of replacing them with a general workspace trait hierarchy.

`operation.rs` owns service-level use case skeletons:

- `setup_workspace`
- `observe_workspace`
- `provision_workspace`
- `execute_workspace`
- `delete_workspace`

`provider.rs` owns resource-oriented provider traits and parameter structs.

`registry.rs` owns static provider registration and provider lookup.

`errors.rs` owns UI-safe provider and workspace operation errors.

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
- `for_provider(provider_id: GpuCloudProviderId) -> Result<&dyn RemoteWorkspaceProvider, RemoteWorkspaceProviderRegistryError>`

Lookup is exact by `GpuCloudProviderId`. Missing providers return `RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id }`.

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

- `WorkspaceObserveError::ExistingVolume`
- `WorkspaceObserveError::ExistingProvisioner`
- `WorkspaceObserveError::ExistingEndpoint`

### provision_workspace

Defines the common orchestration order but does not implement real remote provisioning yet.

Expected future order:

1. Run `observe_workspace` preflight.
2. Create remote volume.
3. Start provisioning runner.
4. Start provisioner worker job.
5. Read provisioner worker status until terminal success or failure.
6. Terminate provisioning runner.
7. Create endpoint.
8. Observe endpoint readiness.
9. Persist Ready workspace state.

Provider implementations expose only resource primitives. They do not duplicate this workflow.

The skeleton returns an explicit `WorkspaceProvisionError::NotImplemented` after validating that the workspace is a remote Draft workspace and the provider exists.

### execute_workspace

Executes only on a Ready remote workspace in a later implementation step.

The skeleton rejects workspaces that are not ready and verifies that a stored endpoint snapshot exists before returning `WorkspaceExecuteError::NotImplemented`.

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

The skeleton includes dependency-order tests using a fake provider and fake catalog collaborator. It does not implement secure storage or real catalog deletion.

## Error Model

Provider API errors are normalized:

- `ProviderApiError::Unauthorized`
- `ProviderApiError::RateLimited`
- `ProviderApiError::Timeout`
- `ProviderApiError::RequestFailed { message }`

Resource-specific provider errors include:

- `ExistingVolume`
- `NonExistingVolume`
- `ExistingProvisioner`
- `NonExistingProvisioner`
- `ExistingEndpoint`
- `NonExistingEndpoint`
- `ProviderApi`

Workspace operation errors preserve resource specificity:

- `WorkspaceObserveError::ExistingVolume`
- `WorkspaceObserveError::ExistingProvisioner`
- `WorkspaceObserveError::ExistingEndpoint`
- `WorkspaceProvisionError::ProviderApi`
- `WorkspaceDeleteError::CleanupFailed`

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

- existing volume returns `WorkspaceObserveError::ExistingVolume`.
- existing provisioner returns `WorkspaceObserveError::ExistingProvisioner`.
- existing endpoint returns `WorkspaceObserveError::ExistingEndpoint`.
- observe does not persist discovered snapshots.

Delete tests:

- delete calls provider cleanup in dependency order: endpoint, provisioner, volume.
- provider not-found cleanup results are treated as already deleted.
- provider cleanup failure prevents local catalog removal.

Error safety tests:

- formatted error output uses sanitized provider messages only.
- provider errors do not expose raw payload fields because raw payload fields are not present in error types.

## Verification

Implementation phases should run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because this design does not change Tauri command contracts, frontend command codegen, build, and lint are not required for the skeleton implementation phase.
