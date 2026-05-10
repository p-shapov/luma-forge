## Why

Native command failures are currently UI-safe but too coarse to debug or guide recovery. Recent Workspace creation debugging showed `invalid_request` can hide a precise client/request contract issue, and the same pattern exists across provider setup, catalog reads, provider inventory reads, Workspace Catalog access, and placement validation.

This change makes command errors more actionable while preserving the security boundary: React receives stable UI-safe codes, messages, retryability, and optional safe recovery metadata, but never receives provider secrets, raw keyring data, SQL internals, raw provider response bodies, or raw persisted Workspace JSON.

## What Changes

- Expand the generated native command error contract with more specific UI-safe error codes for request validation, Workspace Setup placement validation, catalog/profile reads, Provider API failures, keyring/stored-key state, and Workspace Catalog failures.
- Add optional structured, UI-safe error metadata so React can identify the affected command, field, or recovery action without parsing human-readable messages.
- Refine `create_workspace` validation errors so invalid Workspace UUID, missing Workspace name, invalid metadata, and specific invalid Placement Plan causes are distinguishable.
- Refine bundled catalog read errors so Workflow Catalog, Provisioning Profiles, and Endpoint Profiles fail with the correct catalog/profile-specific code.
- Refine Provider Setup and Provider Inventory errors so submitted-key validation, stored-key corruption, provider authorization, provider network/API failures, malformed provider responses, and unavailable identity/inventory data do not collapse into the same code.
- Refine Workspace Catalog read/write errors so local storage access, migration failure, query failure, corruption, and row/schema mismatch can be distinguished safely at the command boundary.
- Update React command failure presentation to use `NativeCommandErrorCode` and safe metadata for clear user-facing copy and recovery affordances.
- Update generated command bindings and spec reference artifacts to match the new command error contract.

## Capabilities

### New Capabilities

### Modified Capabilities

- `native-boundaries`: generated command errors become more specific and may include optional safe metadata while preserving secret isolation and command-boundary ownership.
- `gpu-cloud-provider-setup`: Provider Setup commands expose clearer UI-safe errors for submitted keys, stored keys, identity validation, keyring access, delete-missing state, and provider transport/response failures.
- `workspace-setup`: Workspace Setup read and mutation commands expose clearer UI-safe errors for catalog reads, provider inventory reads, Workspace request validation, placement validation, and Workspace Catalog access.

## Impact

- Affected native command contract: `NativeCommandError`, `NativeCommandErrorCode`, generated TypeScript bindings, and spec reference native contracts.
- Affected native modules: `src-tauri/src/commands`, `src-tauri/src/provider_setup`, `src-tauri/src/workspace_setup`, `src-tauri/src/provider`, `src-tauri/src/bundled_catalog`, `src-tauri/src/workspace_catalog`, and related tests.
- Affected frontend modules: command-console error presentation in `src/pages/home` and any shared UI/error helpers introduced for native command failures.
- No provider secrets, Provider API Keys, raw keyring values, raw provider transport bodies, SQLite internals, raw SQLx errors, or raw Workspace JSON may be exposed to React.
- No database format migration is required.
