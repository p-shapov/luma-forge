## 1. Inventory Current Native Module Names

- [x] 1.1 List native Rust files whose names repeat their parent module or provider context.
- [x] 1.2 Identify which modules have a clear primary implementation that should move into `mod.rs`.
- [x] 1.3 Confirm the final role-based target names for commands, bundled catalog, provider, RunPod, provider setup, workspace setup, workspace catalog, and secrets modules.
- [x] 1.4 Confirm all current `crate::bundled` imports and module declarations that need to become `crate::bundled_catalog`.

## 2. Standardize Application And Infrastructure Modules

- [x] 2.1 Rename Provider Setup secondary files to role-based names and move `ProviderSetupService` primary code into `provider_setup/mod.rs`.
- [x] 2.2 Rename Workspace Setup secondary files to role-based names and move `WorkspaceSetupService` primary code into `workspace_setup/mod.rs`.
- [x] 2.3 Rename Workspace Catalog files to role-based names while keeping repository and SQLite responsibilities separate.
- [x] 2.4 Rename `src-tauri/src/bundled` to `src-tauri/src/bundled_catalog` and rename its files to role-based names while preserving reader, parser, and error responsibilities.
- [x] 2.5 Rename Secret Store files to role-based names while preserving the secret store public surface.

## 3. Standardize Provider And Command Modules

- [x] 3.1 Rename provider registry, provider error, and provider tests to role-based names.
- [x] 3.2 Move `RunPodClient` primary code into `provider/runpod/mod.rs` and rename RunPod contracts, mapper, and tests to role-based names.
- [x] 3.3 Rename command boundary files to role-based names and preserve command builder, bindings, shared provider contracts, and command error responsibilities.
- [x] 3.4 Move command submodule handler code into the relevant command submodule `mod.rs` files and rename command contracts/tests to role-based names.

## 4. Update Paths And Public Surfaces

- [x] 4.1 Update `mod` declarations, `pub use` statements, and direct imports to reference the new role-based module names.
- [x] 4.2 Update `#[path]` attributes for renamed test files.
- [x] 4.3 Preserve concise owning-module imports for primary public types, such as provider setup services and provider clients.
- [x] 4.4 Use `rg` to confirm obsolete prefixed native module names no longer appear in production imports or module declarations.
- [x] 4.5 Use `rg` to confirm production code no longer imports bundled catalog infrastructure through `crate::bundled`.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Review generated/frontend contract output expectations and confirm no user-facing command or payload changes were introduced.
