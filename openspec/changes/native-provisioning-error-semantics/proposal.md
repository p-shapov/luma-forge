## Why

Workspace Provisioning now spans local catalog persistence, secure secret storage, provider resource mutation, provisioner worker communication, and durable recovery metadata. Phase 1 needs a broader native error-semantics contract so immediate command errors, persisted `WorkspaceProvisioningFailure` records, and frontend recovery actions remain granular, stable, and actionable.

## What Changes

- Define which failures should return immediate command errors versus which failures should be persisted as `WorkspaceProvisioningFailure` records on the Workspace.
- Preserve granular Workspace Catalog errors across Workspace Provisioning and Workspace Resources instead of collapsing them into `WorkspaceCatalogUnavailable`.
- Clarify `WorkspaceResourceError` as the resource-operation boundary for catalog/persistence failures, secret/keyring failures, provider API failures, provider resource lifecycle failures, provider operation uncertainty, and provisioner worker token lifecycle failures.
- Clarify `WorkspaceProvisioningError` as the orchestration and command failure boundary for workspace identity/state failures, catalog/persistence failures, secret/keyring failures, transient provider/worker failures, and resource-operation failures that should escape as command errors.
- Keep provider resource corruption, missing resources, orphaned resources, cleanup failures, and provider operation uncertainty represented as persisted Workspace recovery state when the Workspace needs user or cleanup recovery.
- Keep frontend-facing `NativeCommandErrorCode`, `reason`, `retryable`, and `recovery_action` stable, granular, implementation-safe, and free of raw SQLite, reqwest, keyring, RunPod, provider response, or secret details.
- Add regression tests for catalog error preservation, `WorkspaceResourceError -> WorkspaceProvisioningError` mapping, `WorkspaceProvisioningError -> NativeCommandError` mapping, command-error versus persisted-failure behavior, and provisioner worker token lifecycle failure behavior around provisioning pod creation.

## Capabilities

### New Capabilities

- `native-provisioning-error-semantics`: Defines native provisioning/resource error boundaries, command-error versus persisted-failure behavior, and recovery-safe token lifecycle semantics.

### Modified Capabilities

- `native-boundaries`: Command mapping must expose stable UI-safe error metadata while preserving app-owned provisioning/resource error categories internally.
- `workspace-setup`: Workspace Catalog categories must remain distinguishable for storage, migration, query, corrupt data, schema mismatch, and generic unavailable access.
- `workspace-provisioning`: Provisioning orchestration must distinguish immediate command failures from persisted Workspace recovery failures and preserve resource/error semantics through sync, cancellation, and worker preparation.
- `workspace-resources-provider-polymorphism`: Workspace Resources must act as the provider-resource operation boundary and preserve provider, catalog, secret, lifecycle, uncertainty, and token lifecycle categories without leaking provider-specific details.

## Impact

- Affected native modules include Workspace Setup error definitions, Workspace Resources error definitions and provider adapters, Workspace Provisioning orchestration and resource mapping, Provisioner Worker token lifecycle code, persisted provisioning failure helpers, and Tauri command error mappers.
- Affected command contract semantics include `NativeCommandErrorCode`, `message`, `retryable`, `reason`, and `recovery_action` values for provisioning/resource/catalog failures.
- Affected durable state behavior includes when Workspace Provisioning persists `WorkspaceProvisioningFailure` versus returning an immediate command error without mutating Workspace state.
- Affected tests include native Rust regression coverage for mapping boundaries, persisted failure behavior, command error safety, and token lifecycle behavior.
- No frontend UI implementation, full provisioning workflow redesign, raw low-level error exposure, async keyring refactor, new provider support, or broad production `expect` replacement is included.
