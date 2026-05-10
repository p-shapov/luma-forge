## Context

The Workspace Catalog stores indexed SQLite columns plus a serialized domain `Workspace` JSON payload. That payload currently embeds selected Workflow Preset, Provisioning Profile, and Endpoint Profile objects as creation-time snapshots.

The observed failure is caused by a previously persisted row whose `workspace_json` contains older embedded catalog/profile shapes. The SQLite file opens, the schema exists, and integrity checks pass, but deserializing the JSON into the current Rust domain model fails. The repository maps that failure to `workspace_catalog_unavailable`, so `get_workspace_catalog` fails when it decodes rows and `create_workspace` can fail when it re-reads or checks an affected row.

## Goals / Non-Goals

**Goals:**

- Make Workspace Catalog initialization run durable SQLite/data migrations before read or write commands use persisted Workspace records.
- Introduce explicit Workspace Catalog persistence versioning.
- Repair known legacy Workspace JSON payloads written by earlier development builds when the selected catalog/profile ids still exist in the bundled catalogs.
- Keep React as a consumer of authoritative native state without adding frontend-side migration logic.
- Preserve the existing UI-safe command error contract.

**Non-Goals:**

- Do not add a hosted backend or provider-side migration.
- Do not migrate provider resources or provisioning state.
- Do not expose raw SQLite, JSON, provider, or keyring diagnostics to React.
- Do not silently return partial Workspace Catalog data when a persisted row cannot be made authoritative.

## Decisions

1. Version Workspace Catalog persistence with a native-owned SQLite version marker.

   Use a catalog-level migration runner during `SqliteWorkspaceCatalog` initialization. The runner should create missing schema, read the current persistence version, apply ordered migrations in a transaction, and write the target version only after successful completion.

   Alternative considered: delete or ignore unreadable rows. That would unblock reads but would silently discard user-owned state and violate the existing requirement that returned catalog data is authoritative.

2. Use bundled catalogs as the compatibility source for legacy embedded catalog/profile snapshots.

   The migration for current legacy data should parse `workspace_json` as `serde_json::Value`, read the selected Workflow Preset, Provisioning Profile, and Endpoint Profile ids, and replace the embedded selected objects with the current bundled definitions for those ids. This repairs known shape drift such as missing model asset `install` data and nested legacy `docker_image` fields while preserving Workspace id, name, provider, lifecycle, selected datacenter, selected GPU, storage size, and resource snapshots.

   Alternative considered: hand-patch individual JSON fields. That is more brittle because each future catalog/profile model change needs another ad hoc transform.

3. Validate after every migration.

   After a row is transformed, deserialize it into the current domain `Workspace`, run domain validation, and verify indexed row consistency before committing. If any row cannot be migrated or validated, fail the migration and keep the previous data unchanged.

   Alternative considered: best-effort row migration with partial success. That would make command results depend on which rows happened to migrate and would make failure recovery harder to reason about.

4. Keep command responses compatible and improve native diagnostics only.

   Commands should continue returning `workspace_catalog_unavailable` for unrecoverable migration failures. Native logs may include the Workspace id, migration version, and failure category, but must not include provider secrets or raw provider credentials.

   Alternative considered: add a new generated command error code. That is useful later if the UI needs a dedicated recovery action, but it is not necessary to fix the current failure and would expand the frontend contract.

## Risks / Trade-offs

- Legacy row references a catalog/profile id that no longer exists -> fail migration with `workspace_catalog_unavailable`; mitigation: log the missing id category and add a later user-facing recovery flow if needed.
- Migration changes embedded catalog/profile snapshots to current bundled definitions -> this intentionally updates stale creation-time catalog data for draft Workspaces; mitigation: scope the migration to known pre-versioned rows and validate placement compatibility afterward.
- A migration bug could corrupt local data -> run migrations in SQLite transactions and keep tests around representative legacy rows.
- Future domain changes can repeat the problem -> require each persistence-shape change to add a migration and bump the catalog persistence version.

## Migration Plan

1. Add migration infrastructure to Workspace Catalog initialization.
2. Treat existing unversioned catalogs as version `0`.
3. Add version `1` migration that rewrites known legacy Workspace JSON payloads by rehydrating selected catalog/profile objects from bundled catalogs by id.
4. Commit the version bump only after all rows deserialize, validate, and pass indexed row consistency checks.
5. Add tests for an empty new catalog, a current catalog, a legacy row matching the observed stale shape, and an unmigratable legacy row.
6. No rollback migration is required for app runtime; backup or external restore remains the local development rollback path.

## Open Questions

- Should a future UI recovery flow allow users to archive or remove unmigratable local Workspaces instead of treating the whole catalog as unavailable?
- Should Workspace persistence eventually store selected catalog/profile ids plus versions instead of full embedded objects to reduce future migration pressure?
