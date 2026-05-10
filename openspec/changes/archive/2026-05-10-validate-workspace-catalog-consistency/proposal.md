## Why

Workspace Catalog rows store indexed columns alongside a serialized `workspace_json` payload, but current read paths trust only the serialized payload. This leaves the durable catalog able to hide internal row/payload mismatches even though Workspace Setup requires re-read Workspace records to be authoritative and internally consistent.

This should be fixed before provisioning adds more catalog reads, filters, updates, and recovery behavior that may rely on indexed columns.

## What Changes

- Validate Workspace Catalog row consistency whenever a persisted Workspace is read from SQLite.
- Reject Workspace Catalog reads and Workspace creation re-reads with `workspace_catalog_unavailable` when indexed row columns disagree with the serialized Workspace payload.
- Add focused repository tests for mismatched row/payload fields.
- Preserve the existing public command response shape and generated frontend contract.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `workspace-setup`: Clarify that Workspace Catalog reads and create re-reads must reject internally inconsistent row/payload data instead of returning it as authoritative.
- `native-boundaries`: Strengthen the Workspace persistence boundary so denormalized indexed columns must remain consistent with serialized Workspace payloads on every read, not only during normal insert tests.

## Impact

- Affected native code: SQLite-backed Workspace Catalog repository and its tests.
- Affected behavior: corrupt or manually inconsistent Workspace Catalog rows become read failures instead of being silently accepted.
- Affected APIs: none; existing command names, request types, response types, and error code shape remain unchanged.
- Dependencies: none.
