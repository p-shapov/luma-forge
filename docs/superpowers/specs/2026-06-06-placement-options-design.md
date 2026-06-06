# Placement Options Design

## Context

The active native backend uses `remote_workspace` as the application boundary for remote workspace setup, lifecycle operations, deletion, execution, and remote provider integration. Placement options are needed before creating a `RemotePlacementPlan`, so they belong to the remote workspace setup flow rather than a separate top-level service.

Provider adapters will own their credential dependencies. `RemoteWorkspaceService` must not know about `secrets_storage` for this flow.

## Decision

Add placement option retrieval to the existing `RemoteWorkspaceService`.

`RemoteWorkspaceService` will expose a service method that accepts a `GpuCloudProviderId`, resolves the provider through `RemoteWorkspaceProviderRegistry`, and delegates to the provider. The returned value is `RemotePlacementOptions`.

Provider implementations will expose placement option retrieval through a dedicated provider capability trait. `RemoteWorkspaceProvider` will include that capability alongside the existing remote volume, provisioner, and endpoint capabilities.

## Target Shape

```rust
pub trait RemotePlacementOptionsProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>>;
}

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

The service method will have this responsibility:

```rust
pub async fn get_provider_placement_options(
    &self,
    provider_id: GpuCloudProviderId,
) -> Result<RemotePlacementOptions, RemoteWorkspaceError> {
    let provider = self.provider_registry.for_provider(provider_id)?;
    provider.get_provider_placement_options().await
}
```

## Data Flow

1. The Tauri command receives a typed request containing `gpu_cloud_provider_id`.
2. The command calls `RemoteWorkspaceService::get_provider_placement_options`.
3. The service resolves the provider from `RemoteWorkspaceProviderRegistry`.
4. The provider retrieves or derives `RemotePlacementOptions`, including any credential-backed provider calls.
5. The command returns only UI-safe placement option data.

## Error Handling

Provider resolution failures use the existing `RemoteWorkspaceError::ProviderUnavailable`.

Provider adapter failures are normalized into `RemoteWorkspaceError`, usually through `RemoteWorkspaceError::Provider(ProviderApiError)`. Raw provider responses, request bodies, credentials, tokens, and debug output must not leave provider adapters.

## Why Not A Separate Service

A separate placement service is not justified yet. Placement options currently serve one flow: remote workspace setup. Creating another service would duplicate provider resolution and split a small responsibility away from the boundary that already owns remote workspace setup.

If placement later grows into broader behavior, such as cross-provider scoring, caching, workflow-aware filtering, or reusable validation independent of workspace setup, it can be extracted then.

## Testing

Add focused native backend tests for:

- `RemoteWorkspaceService` returns provider placement options from the selected provider.
- `RemoteWorkspaceService` returns `ProviderUnavailable` when no provider is registered.
- `RemoteWorkspaceProviderRegistry` still selects providers correctly after the new trait bound.
- Provider adapters return only `RemotePlacementOptions` and UI-safe errors.

Command contract tests or generated binding checks should be added when the Tauri command signature is wired to the service.
