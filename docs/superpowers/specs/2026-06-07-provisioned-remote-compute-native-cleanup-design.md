# Provisioned Remote Compute Native Cleanup Design

## Problem Statement

The native layer refactor improved the architecture relative to `main`, but the current remote runtime naming and module shape still leave ambiguity. The code currently uses `remote_workspace` and `WorkspaceRuntime::Remote`, which can be read as a generic abstraction for all remote runtimes. The intended model is narrower: remote compute infrastructure that LumaForge provisions and manages, such as RunPod-like providers with persistent volume, provisioner worker, and endpoint worker resources.

The goal is to make that runtime boundary explicit while preserving feature velocity. This pass should clarify names, split the oversized runtime service into focused internal modules, and restore minimal typed command errors for frontend change safety.

## Repository Findings

The current branch replaced the older native layer with a smaller structure:

- `commands/` contains Tauri command adapters and generated binding-safe DTOs.
- `domain/` contains workspace, placement, provider, workflow preset, runtime contract, and secret domain types.
- `remote_workspace/` contains the current RunPod-like runtime lifecycle.
- `secrets_storage/` owns secure storage and API key identity validation.
- `workflow_catalog/` reads and validates bundled workflow/runtime/provisioner catalogs.
- `workspace_catalog/` owns SQLite persistence.

The main remaining architectural issue is `src-tauri/src/remote_workspace/service.rs`, which is over 3,000 lines and combines public application methods, provisioning step transitions, cancellation, cleanup, contract image resolution, and concurrency coordination.

The current command error contract is very simple:

```rust
NativeCommandError {
    message: String,
}
```

That is easier than the previous full recovery model, but it gives the frontend no stable code to branch on.

## Decisions Captured

- Use `WorkspaceRuntime::ProvisionedRemoteCompute(...)` for the current runtime family.
- Rename the `remote_workspace` module to `provisioned_remote_compute`.
- Keep the module flat. Do not introduce `remote_compute/provisioned/...` nesting.
- Allow internal Rust renames when they clarify the boundary.
- Preserve current command names and behavior unless generated type names naturally change from the runtime rename.
- Split the large service into internal modules, not separate public application services.
- Restore minimal typed command errors with `code` and `message` only.
- Do not design endpoint-worker execution in this pass.
- Do not design RunningHub, local runtime, or a general runtime abstraction in this pass.
- Do not redesign persistence or add migration/versioning work in this pass.
- Keep RunPod endpoint cleanup fetch-only for now; do not persist template metadata in this pass.

## Scope

In scope:

- Rename the current remote runtime family from generic "remote workspace" language to provisioned remote compute language.
- Rename `WorkspaceRuntime::Remote` to `WorkspaceRuntime::ProvisionedRemoteCompute`.
- Rename current runtime domain types and DTOs consistently where needed.
- Rename `src-tauri/src/remote_workspace` to `src-tauri/src/provisioned_remote_compute`.
- Split the runtime service into focused internal modules.
- Add minimal typed native command error codes.
- Regenerate frontend command bindings after command type changes.
- Update focused tests for renamed runtime variants, module behavior, and command error codes.

Out of scope:

- Endpoint-worker execution protocol.
- RunningHub, local runtime, hosted remote runtime, or a generalized runtime trait.
- Workspace persistence schema redesign.
- Compatibility shims for old runtime names.
- Full command error recovery metadata such as `retryable`, `field`, or `recovery_action`.
- Persisting RunPod endpoint template id or broader provider metadata.

## Integration Points and Contracts

Native modules involved:

- `src-tauri/src/domain/workspace.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/types/workspace.rs`
- `src-tauri/src/commands/types/placement.rs`
- `src-tauri/src/commands/workspaces.rs`
- `src-tauri/src/commands/catalog.rs`
- `src-tauri/src/remote_workspace/**`, to be renamed to `src-tauri/src/provisioned_remote_compute/**`
- `src-tauri/src/app/bootstrap.rs`
- `src-tauri/src/app/state.rs`
- `src-tauri/src/lib.rs`

Generated frontend contract:

- `src/generated/commands.ts`

The generated TypeScript contract will change for runtime type names and command error shape. Command names should remain stable:

- `createWorkspace`
- `provisionWorkspace`
- `cancelWorkspaceProvisioning`
- `cleanupWorkspace`
- existing catalog and secret commands

## Architecture Design

The domain runtime variant should become:

```rust
WorkspaceRuntime::ProvisionedRemoteCompute(ProvisionedRemoteComputeWorkspace)
```

The runtime module should become:

```text
src-tauri/src/provisioned_remote_compute/
  mod.rs
  service.rs
  flow.rs
  cleanup.rs
  contracts.rs
  coordination.rs
  helpers.rs
  provider.rs
  registry.rs
  providers/
    mod.rs
    runpod/
      mod.rs
      api.rs
      config.rs
      mapping.rs
      provisioner.rs
```

`service.rs` should remain the public facade for app/command callers. It should expose the same behavior as today:

- setup workspace
- get provider placement options
- provision workspace
- cancel provisioning
- cleanup workspace
- reject execution until endpoint execution is designed

The internal modules should have focused responsibilities:

- `flow.rs`: normal provisioning step transitions.
- `cleanup.rs`: cancellation and cleanup paths.
- `contracts.rs`: provisioner and endpoint image reference resolution from bundled catalogs.
- `coordination.rs`: in-flight workspace guard.
- `helpers.rs`: small state mutation helpers.
- `provider.rs`: provisioned remote compute provider capability traits.
- `registry.rs`: provider lookup.

This split is an internal organization boundary, not a new reusable framework.

## Command Error Design

The command error contract should become:

```rust
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
}
```

The enum should cover current behavior only. Expected codes include:

- `WorkflowCatalogInvalid`
- `WorkspaceStorageUnavailable`
- `WorkspaceStorageQueryFailed`
- `WorkspaceStorageCorrupt`
- `WorkspaceStorageSchemaMismatch`
- `WorkspaceAlreadyExists`
- `WorkspaceNotFound`
- `ProviderUnavailable`
- `ProviderSecretUnavailable`
- `ProviderUnauthorized`
- `ProviderInsufficientPermissions`
- `ProviderRateLimited`
- `ProviderTimeout`
- `ProviderRequestFailed`
- `ProvisioningAlreadyRunning`
- `InvalidProvisioningState`
- `ProvisionerWorkerUnauthorized`
- `ProvisionerWorkerUnavailable`
- `ProvisionerWorkerConflict`
- `ProvisionerWorkerResponseInvalid`
- `ProvisionerWorkerFailed`
- `CommandNotImplemented`

Messages must remain UI-safe and must not include raw secrets, bearer tokens, provider API keys, worker tokens, or provider response bodies.

Do not add retry, field, or recovery metadata in this pass.

## Risks and Constraints

This project is pre-v1. The implementation should update current contracts directly and avoid legacy compatibility layers.

The rename will affect persisted workspace JSON if any local development databases contain old `WorkspaceRuntime::Remote` values. Because the project is pre-v1 and compatibility shims are out of scope, existing local data can be discarded or recreated.

The rename will affect generated TypeScript bindings. The implementation must regenerate command bindings and update frontend references if any exist.

The service split must preserve behavior. This is a structural cleanup, not a lifecycle redesign.

Secrets must remain write-only from the UI perspective. No command response, generated type, persisted workspace snapshot, test fixture, error payload, or log may expose raw credentials.

## Error Handling and Edge Cases

Provider and worker errors should map to stable command error codes while preserving the current generic UI-safe messages.

Invalid lifecycle states should remain represented in the runtime provisioning status where the existing behavior does that today, and should map to `InvalidProvisioningState` at command boundaries when returned as a command error.

Cleanup behavior remains fetch-only for RunPod endpoints. If a future bug shows orphaned endpoint templates cannot be cleaned safely, provider metadata persistence can be reconsidered in a separate design.

Execution remains not implemented. A completed workspace with endpoint resources should continue to return a typed not-implemented command error until the endpoint-worker execution protocol is designed.

## Testing Expectations

Required native verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because command bindings change, also run:

```bash
bun run codegen:commands
bun run build
bun run lint
```

Focused tests should cover:

- `WorkspaceRuntime::ProvisionedRemoteCompute` serialization and DTO mapping.
- Minimal command error code serialization.
- Error mapping from workflow catalog, workspace catalog, secrets storage, provider, provisioning, and worker errors.
- Provider registry still resolves the RunPod provider.
- Provisioning, cancellation, and cleanup behavior remains equivalent after module split.

Do not add tests for RunningHub, local runtime, hosted runtime abstractions, endpoint execution protocol, or persistence migrations in this pass.

## Deferred Questions

- Endpoint-worker execution protocol is deferred because execution flow is not being designed in this pass.
- RunningHub or hosted remote runtime design is deferred until that runtime is actively being implemented.
- Workspace persistence versioning is deferred because this pass prioritizes native feature velocity and avoids compatibility layers.
- RunPod template id persistence is deferred because current cleanup remains fetch-only by decision.

## Assumptions to Validate

- A flat `provisioned_remote_compute` module is clearer than nested `remote_compute/provisioned`.
- `WorkspaceRuntime::ProvisionedRemoteCompute` is explicit enough to prevent future RunningHub-like runtimes from being forced into this lifecycle.
- Minimal command error codes are sufficient for the next frontend workflows.
- The service can be split without changing command behavior or provider behavior.

## Recommended Next Step

`superpowers:writing-plans` — the design direction is approved and concrete enough to produce an implementation plan.
