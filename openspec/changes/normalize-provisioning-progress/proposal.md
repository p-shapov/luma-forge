## Why

Workspace Provisioning currently passes Provisioner Worker percentages through as if they represented total Workspace Provisioning progress. This makes the UI show environment-preparation-local progress as global progress and also blurs the boundary between starting the temporary provisioning pod and doing useful environment preparation work.

## What Changes

- Normalize `WorkspaceProvisioningProgress.percent` in the Native Layer so returned percentages represent total Workspace Provisioning progress.
- Treat Provisioner Worker progress as local to `preparing_environment` and map it into the global `40..90%` range.
- Represent Provisioner Worker startup and readiness lag as `starting_provisioning_pod`, not `preparing_environment`.
- Keep React as a renderer of Native-provided progress instead of recalculating provisioning semantics client-side.
- Keep cancellation cleanup and failed states outside the provisioning-to-ready percentage scale unless a separate cleanup progress model is introduced later.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `workspace-provisioning`: Workspace Provisioning Progress phase and percentage semantics will become total-progress based, with explicit handling for worker startup/readiness and worker-local preparation progress.

## Impact

- Native domain progress derivation in `src-tauri/src/domain/workspace/provisioning_state.rs`.
- Provisioner Worker sync handling in `src-tauri/src/workspace_provisioner/mod.rs` should return worker facts/outcomes without owning workspace phase or percentage semantics.
- Workspace Provisioning state-machine/result mapping should convert worker outcomes into UI-safe `WorkspaceProvisioningProgress`.
- Generated command bindings may update if exported Rust contract metadata changes.
- Frontend progress rendering in `src/pages/home/ui/home-page.tsx` should continue to render Native-provided `percent` without owning phase math.
- Existing Workspace Provisioning and Provisioner Worker tests need updates for the new phase and percentage semantics.
