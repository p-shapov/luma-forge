## Context

The native workspace code currently groups Workspace Setup service code, Workspace Catalog repository contracts, SQLite persistence, and their tests under `src-tauri/src/workspace`. This flat module makes the setup orchestration boundary depend on catalog persistence through sibling files with similar names, even though the two concerns have different ownership:

- Workspace Setup coordinates bundled catalog reads, provider inventory access, secret prerequisites, placement validation, and Draft Workspace creation.
- Workspace Catalog owns persistence contracts and SQLite-backed durable Workspace Catalog reads and writes.

The change is structural. It must preserve existing command behavior, generated frontend contracts, domain models, Workspace Catalog persistence schema, and UI-safe error mapping.

## Goals / Non-Goals

**Goals:**

- Introduce dedicated native module roots for Workspace Setup and Workspace Catalog.
- Move setup-owned files into `src-tauri/src/workspace_setup/`.
- Move catalog-owned files into `src-tauri/src/workspace_catalog/`.
- Update imports across commands, bundled catalogs, provider registry, command error mapping, and tests to reference the new boundaries.
- Keep the Workspace Setup service generic over the Workspace Catalog repository trait.
- Preserve current test coverage and native verification commands.

**Non-Goals:**

- Do not change command names, generated TypeScript DTOs, or command payload semantics.
- Do not change SQLite schema, migration behavior, or Workspace row validation.
- Do not redesign Workspace Setup business logic or Workspace domain models.
- Do not add new persistence abstractions beyond moving the existing repository boundary.

## Decisions

### Use top-level native module directories

Create `workspace_setup` and `workspace_catalog` as top-level native modules under `src-tauri/src/` instead of nested directories under `src-tauri/src/workspace/`.

Rationale: The user-requested direction is to split the current files into separate directories named for each boundary. Top-level modules make imports explicit and avoid keeping a generic `workspace` root that still mixes setup and catalog ownership.

Alternative considered: keep `src-tauri/src/workspace/setup` and `src-tauri/src/workspace/catalog`. This would reduce import churn, but it would preserve the broad `workspace` module as the owner of both boundaries.

### Keep error ownership with Workspace Setup for this change

Move `workspace_setup_error.rs` into the `workspace_setup` module and keep `WorkspaceCatalogRepository` methods returning `WorkspaceSetupError`.

Rationale: The current command contract and repository trait already use `WorkspaceSetupError`, and changing error ownership would broaden the refactor into command error mapping and native-boundary requirements. This change is only about directory/module ownership.

Alternative considered: create a catalog-owned error type and map it in Workspace Setup. That may be cleaner later, but it changes application contracts and adds risk unrelated to the requested split.

### Keep repository contract with Workspace Catalog

Move `WorkspaceCatalogRepository` and `UnavailableWorkspaceCatalog` into the `workspace_catalog` module with the SQLite implementation and tests.

Rationale: Setup orchestrates use cases, but repository traits and unavailable persistence adapters are catalog-boundary concerns. Keeping the trait with the catalog module makes persistence ownership clear while preserving setup's generic dependency on a trait.

Alternative considered: place the repository trait under Workspace Setup as an inbound port. That would make the setup use case own the persistence interface, but it would leave catalog persistence split between setup and catalog modules.

### Move tests with the module they exercise

Move setup tests under `workspace_setup` and catalog tests under `workspace_catalog`. Shared test helpers should either stay close to setup tests if they exercise setup behavior, or be duplicated minimally if needed to avoid cross-boundary test-only coupling.

Rationale: Module-local tests should keep validating the same behavior after the file move, and helper placement should not force production code to expose compatibility modules.

Alternative considered: keep tests in the old `workspace` directory through `#[path]` attributes. That would reduce the move size but leave the old directory as an active owner.

## Risks / Trade-offs

- Import churn can miss less obvious test-only or adapter imports -> mitigate with `rg` for old module paths and `cargo test`.
- `#[path]` test modules can break when files move -> mitigate by colocating test files with their owning modules and updating path attributes.
- Public module moves can temporarily affect generated bindings or command registration imports -> mitigate by preserving command DTO names and running native tests plus clippy/fmt.
- Keeping `WorkspaceSetupError` in setup means the catalog module still depends on a setup-owned error type -> accepted for this scoped refactor to avoid changing error semantics; a later change can introduce catalog-local errors if needed.

## Migration Plan

1. Add `src-tauri/src/workspace_setup/mod.rs` and `src-tauri/src/workspace_catalog/mod.rs`.
2. Move setup-owned source and test files into `workspace_setup`.
3. Move catalog-owned source and test files into `workspace_catalog`.
4. Update `src-tauri/src/lib.rs` or the native module root to export the new modules.
5. Update all imports from `crate::workspace::...` to `crate::workspace_setup::...` or `crate::workspace_catalog::...`.
6. Remove the old `workspace` module if it no longer owns production code.
7. Run `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt`.

Rollback is a file/module move reversal with no data migration because command contracts and SQLite schema are unchanged.

## Open Questions

- None.
