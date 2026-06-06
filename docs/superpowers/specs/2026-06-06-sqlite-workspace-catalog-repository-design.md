# SQLite Workspace Catalog Repository Design

## Summary

Add an active native backend `workspace_catalog` module that persists `WorkspaceCatalog` in SQLite. The module provides an async repository boundary, a pass-through service boundary, schema bootstrap, a SQLite adapter, focused errors, and tests.

This design does not wire Tauri commands, generated frontend bindings, workspace lifecycle flows, app data path resolution, legacy database migration, or a normalized legacy schema.

## Goals

- Persist active `Workspace` values in SQLite through a native repository.
- Keep the service boundary ready for later command and workspace lifecycle wiring.
- Store each workspace as one JSON row keyed by workspace id.
- Keep validation minimal and focused on persistence integrity.
- Preserve the rule that persisted workspace snapshots must remain UI-safe and credential-free.
- Avoid legacy compatibility paths during the pre-v1 refactor.

## Module Layout

Create `src-tauri/src/workspace_catalog/`:

- `mod.rs`
  - Declares the module files.
  - Re-exports the public service, repository trait, SQLite adapter, and error type.

- `errors.rs`
  - Defines `WorkspaceCatalogError`.
  - Keeps persistence errors separate from command errors and workflow errors.

- `repository.rs`
  - Defines the async `WorkspaceCatalogRepository` trait.
  - Uses the existing `crate::shared::AppFuture` boxed future style for trait methods.

- `service.rs`
  - Defines `WorkspaceCatalogService<R>`.
  - Wraps a repository implementation and exposes the same operations.
  - Preserves repository errors without adding workflow logic.

- `schema.rs`
  - Owns SQLite schema bootstrap and version checks.
  - Creates metadata and workspace tables for schema version 1.

- `sqlite.rs`
  - Defines `SqliteWorkspaceCatalogRepository`.
  - Provides explicit async `connect(path)` construction for tests and later app-state wiring.
  - Implements `WorkspaceCatalogRepository` using sqlx async SQLite.

Register the active module from `src-tauri/src/lib.rs` with `pub mod workspace_catalog;`. Do not change Tauri command signatures in this slice.

## Dependencies

Add active backend dependencies needed for async SQLite:

- `sqlx` with SQLite and Tokio runtime support.
- `tokio` for async Rust tests and sqlx runtime support.
- `time` if timestamps are implemented as RFC3339 strings through the `time` crate.

Do not add frontend dependencies or code generation requirements for this slice.

## Repository Interface

The repository trait should expose these operations:

```rust
pub trait WorkspaceCatalogRepository: Send + Sync {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>>;

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>>;

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>>;

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>>;

    fn delete_workspace<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>>;
}
```

Insert and update return the persisted `Workspace` value. The trait takes borrowed input to avoid unnecessary ownership at call sites and returns owned domain values to callers.

## Service Boundary

`WorkspaceCatalogService<R>` should be generic over `R: WorkspaceCatalogRepository` and expose the same operations:

- `list_workspaces`
- `find_workspace_by_id`
- `insert_workspace`
- `update_workspace`
- `delete_workspace`

The service should delegate directly to the repository and return the same `WorkspaceCatalogError` values. It should not perform command mapping, lifecycle orchestration, provider calls, or nested domain validation.

## SQLite Schema

Schema version 1 includes:

```sql
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    workspace_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Schema bootstrap writes a metadata entry for the workspace catalog persistence version, with value `1`. After bootstrap, the adapter verifies that the version is present and supported. An absent or unsupported version is a schema mismatch.

`list_workspaces` must order rows by `created_at ASC`.

## Storage Behavior

Each workspace is stored as one row:

- `id`: `Workspace.id`
- `workspace_json`: full serialized `Workspace` JSON
- `created_at`: RFC3339 timestamp set on insert
- `updated_at`: RFC3339 timestamp set on insert and update

Insert creates a new row and preserves `created_at` as the insertion time. Update replaces `workspace_json`, sets `updated_at` to the update time, and preserves the existing `created_at`.

Reconnect behavior should rely on the SQLite file and should not require any in-memory cache.

## Validation And Integrity

Persistence validation is intentionally minimal:

- Reject blank workspace ids for insert, update, delete, and find input.
- Enforce that a row id equals the deserialized `Workspace.id`.
- Reject duplicate ids on insert.
- Treat invalid workspace JSON as catalog corruption.
- Treat unreadable row shapes or unsupported schema metadata as schema mismatch.
- Do not add a nested workspace domain validator in this slice.

The persisted `Workspace` JSON must not include raw provider credentials, bearer tokens, worker tokens, Hugging Face keys, or future secrets. This matches the current domain expectation that workspace snapshots are UI-safe. Future workspace fields must preserve that rule.

## Error Handling

Define `WorkspaceCatalogError` with:

- `StorageUnavailable`
- `MigrationFailed`
- `QueryFailed`
- `Corrupt`
- `SchemaMismatch`
- `WorkspaceAlreadyExists`
- `WorkspaceNotFound`

Error mapping rules:

- SQLite connection/open failures map to `StorageUnavailable`.
- Schema bootstrap execution failures map to `MigrationFailed`.
- SQL query execution failures map to `QueryFailed`.
- SQLite primary-key or unique constraint conflicts on insert map to `WorkspaceAlreadyExists`.
- Update/delete operations that affect zero rows map to `WorkspaceNotFound`.
- Invalid JSON and row id/JSON id mismatches map to `Corrupt`.
- Missing required columns, invalid column types, or unsupported persistence metadata map to `SchemaMismatch`.

Blank ids are invalid persistence input. They should return `Corrupt` because they would create or address invalid catalog data.

## Legacy Reference Boundaries

The legacy backend may be used only as implementation reference for sqlx patterns and test ideas. Do not restore:

- The legacy normalized schema.
- Legacy workspace lifecycle fields.
- Legacy database migrations.
- Compatibility shims.
- Fallback behavior for old contracts.

The active schema stores one serialized active-domain `Workspace` JSON document per row.

## Testing

Add focused Rust tests for:

- Schema creation on connect.
- Empty list returns `WorkspaceCatalog { workspaces: vec![] }`.
- Insert, list, and find round trip a workspace.
- Duplicate insert returns `WorkspaceAlreadyExists`.
- Update replaces an existing workspace and preserves find/list behavior.
- Delete removes an existing workspace.
- Missing update returns `WorkspaceNotFound`.
- Missing delete returns `WorkspaceNotFound`.
- Corrupt JSON returns `Corrupt`.
- Row id and serialized workspace id mismatch returns `Corrupt`.
- Persistence survives reconnecting to the same SQLite file.
- Service methods delegate repository results and errors without remapping.

Tests may create temporary SQLite files under a temp directory and remove them after completion.

## Verification

Run native verification from the repository root:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Do not run `bun run codegen:commands` for this slice because command signatures and generated frontend bindings are unchanged.

## Out Of Scope

- Tauri command wiring.
- Generated frontend bindings.
- Frontend UI.
- Workspace lifecycle flow integration.
- App data path wiring through Tauri runtime APIs.
- Legacy database migration.
- Normalized relational workspace querying.
- Secrets, provider credentials, bearer tokens, worker tokens, or Hugging Face keys in persisted workspace JSON.
- Full nested domain validation.
