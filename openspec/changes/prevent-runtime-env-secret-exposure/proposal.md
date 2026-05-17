## Why

RunPod serverless template environment variables can contain provider secrets, operator metadata, or other values that are not safe for React. LumaForge currently persists and exports template `runtime_env` through Workspace metadata, which violates the Native Layer secret-safety boundary.

## What Changes

- Remove persisted RunPod endpoint template runtime environment values from Workspace metadata.
- Remove endpoint template runtime environment values from generated command response bindings.
- Keep endpoint template reuse and cleanup metadata based on non-secret identifiers and safe template properties only.
- Treat provider-returned template environment values as observation-only data that must not be stored in the Workspace Catalog, returned to React, logged, or used in diagnostics.
- Update provisioning tests to prove secret-like template env values are discarded before persistence and never appear in command-facing snapshots.

## Capabilities

### New Capabilities

### Modified Capabilities
- `workspace-provisioning`: Require RunPod endpoint template snapshots to persist only UI-safe cleanup and reuse metadata, excluding provider-returned runtime environment values.
- `native-boundaries`: Require generated Workspace command DTOs to exclude RunPod endpoint template environment maps and other provider-returned template env values.

## Impact

- Affected native domain types: `RunPodEndpointTemplateSnapshot` and provisioning snapshot mapping.
- Affected command contract: generated Workspace DTOs and frontend bindings for provider provisioning snapshots.
- Affected persistence: Workspace Catalog JSON/SQLite metadata must tolerate older records with `runtime_env` but omit it on subsequent writes.
- Affected tests: provisioning snapshot tests, command contract/export tests, and any fixture builders that currently set template runtime env.
