## Why

Tauri command handlers currently assemble native dependencies directly, including app data path resolution, SQLite catalog connection, service construction, and coordinator access. This is manageable for existing setup flows, but Workspace Provisioning will need shared catalog access, per-workspace coordination, and consistent recovery behavior across multiple commands.

## What Changes

- Introduce a native application state/service composition boundary managed by Tauri during app startup.
- Move durable infrastructure ownership and service construction out of individual command handlers.
- Keep command handlers focused on request mapping, service invocation, response mapping, and UI-safe error mapping.
- Share Workspace Catalog access through the managed native state instead of opening and migrating SQLite independently inside workspace command handlers.
- Provide a natural home for shared coordinators, including the existing provider setup coordinator and future workspace operation coordination.
- Preserve existing command names, payloads, generated TypeScript bindings, and UI-safe error semantics.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-boundaries`: Define that native dependency composition, shared infrastructure ownership, and operation coordination belong outside command handlers.

## Impact

- Affected native code:
  - `src-tauri/src/lib.rs`
  - `src-tauri/src/commands/**`
  - native service/composition modules added or adjusted under `src-tauri/src/`
  - workspace catalog initialization and reuse
  - provider setup coordinator ownership
- No frontend API changes are intended.
- No provider API behavior changes are intended.
- No database schema changes are intended.
