## Why

Native domain code currently contains a broad `dead_code` allowance around an unused Workspace lifecycle model. This masks unfinished architecture and lets application contracts become the de facto domain model just before Workspace Provisioning adds lifecycle transitions, resource snapshots, cancellation, and failure recovery.

## What Changes

- Remove broad `dead_code` allowances from native source code.
- Permit only narrow, documented `dead_code` allowances for domain vocabulary that is already defined by accepted specs and will be constructed by near-term flows.
- Promote Workspace lifecycle construction into the domain layer, starting with authoritative Draft Workspace creation.
- Keep application/service contracts as serializable service and persistence-facing shapes, but stop hand-constructing lifecycle-bearing Workspace records in services.
- Add explicit mapping between the domain Workspace model and the existing Workspace application contract.
- Remove unused speculative domain types that are not needed by the current behavior.
- Do not implement Workspace Provisioning in this change.
- Do not change generated command payload shapes or perform a SQLite schema migration.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-boundaries`: Require broad unused-code suppressions to be removed, while allowing targeted comments for spec-defined future domain vocabulary.
- `workspace-setup`: Require Draft Workspace creation to be authored through the domain Workspace model before persistence and command response mapping.

## Impact

- Affected native code: `src-tauri/src/domain`, `src-tauri/src/workspace`, and tests around Workspace Setup and workspace contract mapping.
- Affected specs: `native-boundaries` and `workspace-setup`.
- External API impact: none intended. Generated command payloads, serialized Workspace fields, command names, and UI-safe error behavior should remain compatible.
- Persistence impact: no SQLite schema migration intended. Existing Workspace JSON shape should remain compatible.
