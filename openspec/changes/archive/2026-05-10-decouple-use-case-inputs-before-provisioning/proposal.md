## Why

Provider Setup and Workspace Setup application services currently accept generated command DTOs directly. Workspace Provisioning will add lifecycle mutations, provider resource snapshots, sync progress, cancellation, and recovery paths; carrying generated frontend binding concerns into those service APIs would make internal workflow changes more expensive.

## What Changes

- Reclassify existing Provider Setup and Workspace Setup contract types as application/service contracts where practical.
- Keep `serde` derives on existing contracts where they are already useful for parsing, persistence, or stable data snapshots.
- Remove `specta::Type` from application/service contract types so generated frontend binding concerns no longer live in service-owned contracts.
- Add command-owned DTOs under the Tauri command boundary for generated request/response payloads that need `specta::Type`.
- Map command-owned DTOs to and from application/service contracts in command handlers or command-adjacent mappers.
- Preserve existing generated command names, serialized payload fields, error codes, and user-visible behavior.
- Keep this change focused on boundary decoupling; do not implement Workspace Provisioning, lifecycle mutation commands, Provider Resource creation, or schema migrations.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-boundaries`: Require generated frontend binding traits to be owned by command DTOs rather than application/service contracts, with command handlers or command mappers responsible for conversion.

## Impact

- Affected native code: `src-tauri/src/commands`, `src-tauri/src/provider_setup`, `src-tauri/src/workspace`, and generated TypeScript binding export.
- Affected tests: service tests should use application/service contracts; command-boundary tests should cover DTO mapping and generated command compatibility where useful.
- External API impact: none intended. Generated TypeScript command bindings should remain payload-compatible.
- Dependencies and persistence: no new dependencies, no SQLite schema migration, and no Provider API behavior changes.
