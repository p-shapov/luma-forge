## Why

Workspace Provisioning can currently retry some non-idempotent RunPod create operations after an indeterminate provider result without first proving whether the provider accepted the previous request. That can duplicate paid provider resources and leave the Workspace Catalog without enough cleanup metadata to unwind safely.

This change hardens the existing provisioning recovery behavior so Native either adopts exactly one Workspace-correlated provider resource or fails closed with durable failure detail and retained cleanup metadata.

## What Changes

- Treat indeterminate provider create outcomes for RunPod volumes, provisioning pods, serverless templates, and serverless endpoints as unsafe continuation points unless exactly one safe Workspace-correlated resource can be identified.
- Add or complete provider discovery paths needed to adopt Workspace-correlated resources after lost create responses, using stable Workspace-derived provider names and existing resource correlation fields.
- Convert missing tracked provider resources during provisioning refresh into durable `failed` Workspace state with structured `provider_resource_missing` failure detail instead of leaving provisioning stuck behind a command error.
- Ensure shared cleanup deletes the per-workspace Provisioner Worker bearer token even when no active pod snapshot exists.
- Make cancellation report a retryable conflict when another sync currently owns the Workspace instead of returning unchanged provisioning metadata as a successful cancellation.
- Preserve command and generated binding shapes; no frontend contract break is intended.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-provisioning`: Tighten recovery, idempotency, missing-resource, cleanup-token, and cancellation-conflict requirements for existing Workspace Provisioning behavior.

## Impact

- Affected native modules:
  - `src-tauri/src/workspace_provisioning`
  - `src-tauri/src/workspace_resource_cleanup`
  - `src-tauri/src/provider/registry.rs`
  - `src-tauri/src/provider/runpod`
  - `src-tauri/src/commands/error.rs`
- Affected frontend module:
  - `src/pages/home/ui/home-page.tsx`, only if command-conflict presentation or auto-sync behavior needs adjustment.
- Affected tests:
  - Workspace Provisioning service tests.
  - RunPod provider discovery and mapping tests.
  - Cleanup behavior tests.
- No new external service dependency is expected.
