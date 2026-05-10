## Why

Workspace Catalog reads and some Workspace creation paths currently fail when persisted `workspace_json` was written by an older app build whose embedded catalog/profile snapshot shape no longer matches the current Rust domain structs. This blocks users from listing or creating Workspaces after local data drift, even though the SQLite catalog file itself is readable and intact.

## What Changes

- Add a native-owned Workspace Catalog migration boundary that runs before Workspace Catalog data is returned or used for writes.
- Version the Workspace Catalog persistence format so future schema or serialized JSON shape changes have an explicit compatibility path.
- Migrate known legacy Workspace JSON payloads into the current authoritative shape without exposing partial or invalid catalog data.
- Preserve existing UI-safe command behavior: unrecoverable catalog initialization, migration, decoding, or validation failures still return `workspace_catalog_unavailable`.
- Add developer diagnostics for migration failures that do not expose secrets or provider credentials.

## Capabilities

### New Capabilities

### Modified Capabilities

- `workspace-setup`: Workspace Catalog read/create behavior changes to initialize and apply native persistence migrations before decoding or writing Workspace records.

## Impact

- Affected native modules: `src-tauri/src/app_state.rs`, `src-tauri/src/workspace_catalog`, `src-tauri/src/workspace_setup`, and related tests.
- Affected durable state: local SQLite Workspace Catalog under the Tauri app data directory.
- Affected commands: `get_workspace_catalog` and `create_workspace`.
- No frontend command contract change is expected.
- No provider API behavior, keyring behavior, or hosted backend dependency is introduced.
