## Context

The native layer already has ownership-oriented module directories such as `provider_setup`, `workspace_setup`, `workspace_catalog`, `bundled`, `provider`, `provider/runpod`, and `commands`. Inside those directories, many file names still repeat the owning module name:

```text
provider_setup/provider_setup_service.rs
workspace_setup/workspace_setup_error.rs
workspace_catalog/workspace_catalog_repository.rs
bundled/bundled_catalog_reader.rs
provider/runpod/runpod_client.rs
commands/workspace/workspace_handlers.rs
```

That convention made sense while modules were being split out of flatter boundaries, but it now creates redundant paths and weakens the role-based layout. The `bundled` root is also too broad for its actual responsibility: it owns bundled catalog loading, not arbitrary bundled application assets.

The desired shape is for each directory to provide the domain context and for files inside it to be named by responsibility:

```text
workspace_setup/
  mod.rs
  contracts.rs
  error.rs
  tests.rs

workspace_catalog/
  mod.rs
  repository.rs
  sqlite.rs
  tests.rs

bundled_catalog/
  mod.rs
  error.rs
  parser.rs
  reader.rs
  tests.rs

provider/runpod/
  mod.rs
  contracts.rs
  mapper.rs
  tests.rs
```

The change is structural. It must preserve command behavior, generated frontend contracts, domain behavior, persistence schema, provider behavior, and UI-safe error semantics.

## Goals / Non-Goals

**Goals:**

- Rename native Rust files so file names describe only the local role within their parent module.
- Rename `src-tauri/src/bundled` to `src-tauri/src/bundled_catalog`.
- Move primary module implementations into `mod.rs` where a module has one clear central type or behavior.
- Update module declarations, imports, re-exports, test path attributes, and references to the renamed modules.
- Keep public import paths concise where existing module re-exports already support that style.
- Preserve existing ownership boundaries and all user-visible behavior.
- Verify the refactor with native tests, clippy, and formatting.

**Non-Goals:**

- Do not change command names, generated TypeScript DTO names, serialized payloads, or command error codes.
- Do not change SQLite schema, persisted data shape, provider API requests, or provider API response mapping.
- Do not redesign service logic, domain models, validation rules, secret handling, or provider abstractions.
- Do not introduce compatibility shim modules that preserve old redundant file-path names in production code.
- Do not rename frontend files unless they become directly relevant to the native convention.

## Decisions

### Use parent directories for context and child files for role

Within a module directory, child file names will omit repeated parent/module prefixes. For example, `workspace_setup/workspace_setup_error.rs` becomes `workspace_setup/error.rs`, and `bundled/bundled_catalog_parser.rs` becomes `bundled_catalog/parser.rs`.

Rationale: The parent directory already supplies the concept. Repeating it in every file makes paths longer without adding ownership information.

Alternative considered: Keep prefixed file names for searchability. This keeps global filenames unique, but Rust module paths already include the parent context, and role-based names are easier to scan in a nested module tree.

### Rename `bundled` to `bundled_catalog`

The existing `bundled` module will become `bundled_catalog`, with catalog reader/parser/error/test responsibilities inside it.

Rationale: The module currently owns bundled Workflow Catalog, Provisioning Profile, and Endpoint Profile loading. Naming the root `bundled_catalog` makes that ownership explicit and leaves room for future bundled assets that are not catalog infrastructure.

Alternative considered: Keep `bundled` and only rename child files. That would reduce import churn, but it would keep a generic module root whose actual responsibility is narrower than its name.

### Put the primary module implementation in `mod.rs`

When a module has one clear primary responsibility, the central implementation will live in `mod.rs`. Examples include:

- `provider_setup/mod.rs` owning `ProviderSetupService` and `ProviderIdentityGateway`.
- `workspace_setup/mod.rs` owning `WorkspaceSetupService`, `ProviderInventoryGateway`, and `WorkspaceSetupCatalogReader`.
- `provider/runpod/mod.rs` owning `RunPodClient`.
- command submodule `mod.rs` files owning their local command handlers when the submodule primarily exists to expose those handlers.

Secondary responsibilities remain in role files such as `error.rs`, `contracts.rs`, `mapper.rs`, `repository.rs`, and `sqlite.rs`.

Rationale: The module root should contain the module's main behavior and public surface. This avoids a `mod.rs` that only points to a same-named service file.

Alternative considered: Keep every implementation in role files and use `mod.rs` only as an index. That is reasonable for large modules with multiple equal peers, but it preserves the extra indirection for modules that have a single obvious center.

### Keep public API behavior stable while reducing internal path stutter

Existing re-exports should be preserved or improved so call sites can import important types from the owning module root where appropriate, for example `crate::provider_setup::ProviderSetupService`.

For implementation details that should remain scoped, call sites should use role-based module paths, for example `crate::workspace_catalog::repository::WorkspaceCatalogRepository` or `super::contracts::CreateWorkspaceRequest`.

Rationale: Public module roots should express ownership. Role-based child modules should remain available where direct access is appropriate, especially for traits, persistence implementations, command DTOs, and tests.

Alternative considered: Re-export everything from every `mod.rs`. That produces short imports but hides the distinction between stable public surface and internal implementation details.

### Treat tests as role files

Colocated module tests should be renamed to `tests.rs`, with `#[path = "tests.rs"]` where needed. Test helpers should remain near the module that owns the behavior they exercise.

Rationale: Test files are also module-local roles. Keeping repeated names such as `workspace_setup_tests.rs` would leave the convention incomplete.

Alternative considered: Leave test file names unchanged to reduce churn. This would avoid some path updates, but it would make the convention inconsistent in the exact files developers commonly scan during refactors.

## Risks / Trade-offs

- Import churn can miss test-only paths or `#[path]` attributes -> mitigate with `rg` for old prefixed module names and `cargo test`.
- Moving primary code into `mod.rs` can create large module roots over time -> mitigate by only moving the central implementation now and keeping secondary responsibilities in role files.
- Command handler submodules may become less symmetrical if handlers move into `mod.rs` while contracts remain in `contracts.rs` -> accepted because each command submodule primarily exists to expose handlers.
- Some role names may collide conceptually across directories, such as many `error.rs` files -> accepted because Rust module paths and editor breadcrumbs preserve parent context.

## Migration Plan

1. Rename `src-tauri/src/bundled` to `src-tauri/src/bundled_catalog`.
2. Rename role files within each affected native module to remove repeated parent/module prefixes.
3. Move each module's central implementation into `mod.rs` where the module has a clear primary behavior.
4. Update `mod` declarations, `pub use` statements, imports, and `#[path]` test attributes.
5. Use `rg` to confirm old redundant module names and `crate::bundled::` imports are gone from production code and expected test paths.
6. Run `cargo test`.
7. Run `cargo clippy --fix --allow-dirty --allow-staged`.
8. Run `cargo fmt`.

Rollback is a file/module rename reversal. No data migration is required because generated command contracts, provider behavior, and SQLite persistence are unchanged.

## Open Questions

- None.
