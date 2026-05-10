## Why

Workspace setup orchestration and Workspace Catalog persistence currently share the same `src-tauri/src/workspace` module root, even though they represent separate application boundaries. Splitting them into dedicated module directories will make ownership clearer before more provisioning and catalog behavior is added.

## What Changes

- Move Workspace Setup service, contracts, errors, and tests under a dedicated `workspace_setup` native module directory.
- Move Workspace Catalog repository traits, SQLite implementation, and tests under a dedicated `workspace_catalog` native module directory.
- Update native imports and module exports so command handlers, bundled catalog readers, provider adapters, and tests use the new module boundaries.
- Preserve existing command behavior, generated frontend contracts, domain models, persistence schema, and error semantics.
- Keep compatibility shims only if needed for a low-risk transition, and remove obsolete flat `workspace::*` module paths from production code.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `native-boundaries`: Clarify that Workspace Setup orchestration and Workspace Catalog persistence are separate native module boundaries.

## Impact

- Affected code: `src-tauri/src/workspace/workspace_setup_service.rs`, `src-tauri/src/workspace/workspace_catalog_repository.rs`, `src-tauri/src/workspace/workspace_catalog_sqlite.rs`, related workspace setup/catalog tests, and native imports across commands, bundled catalog, provider registry, and command error mapping.
- APIs: No user-facing command names, request/response payloads, generated TypeScript bindings, or persistence schema changes.
- Dependencies: No new dependencies.
- Verification: Run `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt` for native changes.
