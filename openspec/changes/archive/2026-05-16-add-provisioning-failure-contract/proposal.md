## Why

Workspace Provisioning currently exposes terminal failure mostly through `WorkspaceLifecycleState::Failed`, while provider and worker errors are represented separately as command errors. The previous progress `message` slot was too unstructured to be the failure contract. This makes it unclear which failures should become durable workspace state, which should remain retryable command failures, and what UI-safe failure detail React can rely on after a workspace is already failed.

## What Changes

- Add a structured, UI-safe provisioning failure contract derived from Native-owned workspace state instead of using an untyped progress message as an error channel.
- Define when provider API failures remain `NativeCommandError` responses and when they must persist the workspace lifecycle as `failed`.
- Preserve cleanup metadata whenever a terminal provisioning failure is recorded.
- Ensure terminal worker, provider resource, readiness, and unsafe-continuation failures expose stable failure codes, phases, retryability, and recovery guidance without exposing secrets or raw provider/worker diagnostics.
- **BREAKING**: Generated frontend bindings for `WorkspaceProvisioningProgress` and `Workspace` gain structured failure fields, and `WorkspaceProvisioningProgress.message` is removed as a provisioning failure channel.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-provisioning`: Clarifies durable failure state, provider API error lifecycle effects, and structured UI-safe provisioning failure reporting.

## Impact

- Affected Rust domain models: `src-tauri/src/domain/workspace/`.
- Affected provisioning orchestration: `src-tauri/src/workspace_provisioning/`.
- Affected command contracts and generated bindings: `src-tauri/src/commands/workspace_provisioning/`, `src-tauri/src/commands/error.rs`, generated TypeScript command types.
- Affected frontend consumers: React provisioning progress rendering and any sync-loop failure handling.
- No new provider dependency or hosted backend is introduced.
