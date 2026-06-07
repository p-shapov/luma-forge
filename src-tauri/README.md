# Native Backend

This directory contains the active Tauri native backend. Business workflows live in application services, Tauri commands stay as adapters, and domain models stay independent from Tauri runtime APIs, persistence adapters, UI concerns, and provider SDK details.

## Workspace Runtimes

A `Workspace` owns common workspace identity and workflow selection. `WorkspaceRuntime` describes how that workspace is operated at runtime.

At the moment, only the remote runtime is available: `Remote(RemoteWorkspace)`. It represents provider-backed GPU infrastructure and remote provider resources. Future runtimes should be added only when they have a clear owner service and operation boundary.

When adding a new workspace runtime, do not route it through `remote_workspace`. Update the `remote_workspace` guard in `src/remote_workspace/service.rs` to reject non-remote runtimes with an explicit `RemoteWorkspaceError`, and add tests for observe, provision, execute, and delete rejection.

## Remote Workspace

`remote_workspace` is the native backend boundary for remote workspace setup, observation, provisioning, execution, deletion, and remote provider integration. It owns the service-level workflow surface and the source-level extension point for remote GPU providers.

Provider adapters must return only UI-safe errors and snapshots. Do not return raw provider responses, request bodies, API keys, bearer tokens, worker tokens, Hugging Face keys, credential-bearing URLs, SDK debug output, or environment dumps.

### Adding A Remote Provider

To add a provider:

1. Add the provider id to `GpuCloudProviderId` in `src/domain/provider.rs`.
2. Add a provider-specific module under `src/remote_workspace/providers/<provider_name>/`.
3. Implement the resource traits from `src/remote_workspace/provider.rs`:
   - `RemoteVolumeProvider`
   - `RemoteProvisionerProvider`
   - `RemoteEndpointProvider`
   - `RemoteWorkspaceProvider`
4. Normalize provider SDK/API failures into `RemoteWorkspaceError`.
5. Make sure every returned error is UI-safe before it leaves the provider adapter.
6. Register the adapter in `RemoteWorkspaceProviderRegistry` in `src/remote_workspace/registry.rs`.
7. Add registry selection tests.
8. Add provider contract tests for resource behavior:
   - observe returns `Ok(None)` when a matching resource does not exist.
   - create returns the matching `Existing*` error when a resource already exists.
   - delete returns the matching `NonExisting*` error when a known resource is already gone.

Provider implementations should expose resource primitives only. They should not duplicate the full provisioning workflow; orchestration belongs in `RemoteWorkspaceService`.

## Verification

For native backend changes, run from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

If Tauri command contracts or exported Specta types change, also run:

```bash
bun run codegen:commands
bun run build
bun run lint
```
