## 1. Module Structure

- [x] 1.1 Create `src-tauri/src/workspace_setup/mod.rs` exporting setup service, setup contracts, and setup error modules.
- [x] 1.2 Create `src-tauri/src/workspace_catalog/mod.rs` exporting repository and SQLite catalog modules.
- [x] 1.3 Update the native module root to expose `workspace_setup` and `workspace_catalog`.

## 2. Move Workspace Setup Code

- [x] 2.1 Move `workspace_setup_service.rs`, `workspace_setup_contracts.rs`, `workspace_setup_error.rs`, and setup tests into `src-tauri/src/workspace_setup/`.
- [x] 2.2 Update Workspace Setup service imports to depend on `crate::workspace_catalog::workspace_catalog_repository::WorkspaceCatalogRepository`.
- [x] 2.3 Keep Workspace Setup behavior unchanged for catalog reads, provider inventory reads, placement validation, and Draft Workspace creation.

## 3. Move Workspace Catalog Code

- [x] 3.1 Move `workspace_catalog_repository.rs`, `workspace_catalog_sqlite.rs`, and catalog tests into `src-tauri/src/workspace_catalog/`.
- [x] 3.2 Update Workspace Catalog imports to use `crate::workspace_setup::workspace_setup_error::WorkspaceSetupError`.
- [x] 3.3 Preserve the existing SQLite schema, row consistency validation, duplicate handling, and authoritative re-read behavior.

## 4. Update Call Sites

- [x] 4.1 Update command error mapping, workspace command handlers, and workspace command contracts to use the new setup and catalog module paths.
- [x] 4.2 Update bundled catalog reader imports to use `workspace_setup::workspace_setup_service::WorkspaceSetupCatalogReader`.
- [x] 4.3 Update provider registry and provider tests to use `workspace_setup` gateway and error paths.
- [x] 4.4 Update any remaining tests or helpers that import old flat `crate::workspace::workspace_*` paths.

## 5. Cleanup And Verification

- [x] 5.1 Remove the obsolete flat `src-tauri/src/workspace` module if no production code remains under it.
- [x] 5.2 Use `rg` to confirm production imports no longer reference obsolete `crate::workspace::workspace_setup_*` or `crate::workspace::workspace_catalog_*` paths.
- [x] 5.3 Run `cargo test`.
- [x] 5.4 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.5 Run `cargo fmt`.
