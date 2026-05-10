## 1. Migration Infrastructure

- [x] 1.1 Add a Workspace Catalog migration runner that executes during SQLite catalog initialization before repository reads or writes.
- [x] 1.2 Add a native persistence version marker for the Workspace Catalog and treat existing unversioned catalogs as version `0`.
- [x] 1.3 Ensure schema creation and versioned data migrations run in a transaction and only record the target version after successful validation.

## 2. Legacy Workspace JSON Migration

- [x] 2.1 Add a version `1` migration for legacy Workspace JSON payloads that parses rows as JSON values before domain deserialization.
- [x] 2.2 Rehydrate selected Workflow Preset, Provisioning Profile, and Endpoint Profile objects from current bundled catalogs by selected id.
- [x] 2.3 Preserve Workspace identity, name, provider id, lifecycle state, placement choices, storage size, resource snapshots, and environment preparation timestamp during migration.
- [x] 2.4 Reject migration when required selected catalog/profile ids are missing, JSON is malformed, or the migrated Workspace fails domain validation.

## 3. Repository Integration

- [x] 3.1 Pass the bundled catalog/profile compatibility source into Workspace Catalog initialization from native app state.
- [x] 3.2 Keep `get_workspace_catalog` returning only fully migrated and validated authoritative Workspace records.
- [x] 3.3 Ensure `create_workspace` applies migrations before duplicate checks, inserts, and post-insert re-reads.
- [x] 3.4 Keep command error mapping compatible by returning `workspace_catalog_unavailable` for unrecoverable migration failures.

## 4. Diagnostics And Tests

- [x] 4.1 Add native diagnostics for migration failure categories without logging secrets or provider credentials.
- [x] 4.2 Add tests for a new empty catalog and an already-current catalog.
- [x] 4.3 Add tests for the observed legacy stale Workspace JSON shape and assert the migrated catalog reads successfully.
- [x] 4.4 Add tests for unmigratable legacy rows and assert no persistence version bump is recorded.
- [x] 4.5 Add tests that duplicate Workspace creation after migration returns `workspace_already_exists` instead of `workspace_catalog_unavailable`.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
