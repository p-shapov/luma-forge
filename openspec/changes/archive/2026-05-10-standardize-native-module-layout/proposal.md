## Why

Native Rust modules currently use file names that repeat their parent module names, such as `provider_setup_service.rs`, `workspace_catalog_sqlite.rs`, and `runpod_client.rs`. This creates noisy paths, weakens the role-based module convention, and makes ownership harder to scan as more native workflows are added.

## What Changes

- Standardize native module file names so files are named by their local role, such as `error.rs`, `service.rs`, `reader.rs`, `parser.rs`, `repository.rs`, `sqlite.rs`, `contracts.rs`, `handler.rs`, `mapper.rs`, and `tests.rs`.
- Rename the native `bundled` module root to `bundled_catalog` so the module name reflects that it owns bundled catalog loading rather than generic bundled assets.
- Move each module's primary implementation into that module's `mod.rs` when the parent module has a clear primary responsibility.
- Update module declarations, imports, test path attributes, and public re-exports to use the new role-based file names.
- Preserve existing ownership boundaries between commands, application services, domain modules, provider implementations, bundled catalog infrastructure, secret storage, and persistence.
- Preserve existing command names, generated TypeScript bindings, SQLite schema, domain behavior, provider behavior, and error semantics.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `native-boundaries`: Add a module-layout convention that requires role-based file names, `bundled_catalog` ownership naming, and primary module code in `mod.rs` where appropriate.

## Impact

- Affected code: `src-tauri/src/**` module declarations, imports, re-exports, colocated tests, path attributes for renamed native Rust files, and imports from the current `bundled` module.
- APIs: No user-facing command names, generated frontend payloads, SQLite schema, provider API behavior, or persisted state changes.
- Dependencies: No new dependencies.
- Verification: Run `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt` for native changes.
