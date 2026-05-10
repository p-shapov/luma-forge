## Context

Workspace Catalog persistence currently stores a compact set of indexed SQLite columns (`id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, `workflow_preset_id`, timestamps) plus a serialized `workspace_json` payload. The insert path derives indexed values from the Workspace being stored, then re-reads the Workspace before returning success.

The read paths only select and decode `workspace_json`. That makes the serialized payload the practical source of truth, while the indexed columns can silently diverge. This is weaker than the Workspace Setup flow and native boundary specs, which describe durable Workspace Catalog state as authoritative and internally consistent.

## Goals / Non-Goals

**Goals:**

- Treat row/payload mismatch as Workspace Catalog unavailability.
- Validate consistency for both catalog listing and post-insert re-read.
- Keep the serialized Workspace payload as the returned API object.
- Keep existing public command contracts unchanged.
- Add tests that prove inconsistent durable rows are rejected.

**Non-Goals:**

- Normalize the entire Workspace Catalog schema.
- Add data repair, migration, or user-facing recovery flows.
- Change frontend behavior or generated TypeScript contract shape.
- Add provider-specific Workspace Catalog behavior.

## Decisions

### Validate on read, not only on insert

Every repository read should select the indexed columns needed to prove consistency with `workspace_json`, decode the payload, and compare the duplicated fields before returning a Workspace.

Fields to validate:

- row `id` equals `workspace.id`
- row `name` equals `workspace.name`
- row `gpu_cloud_provider_id` equals `workspace.gpu_cloud_provider_id`
- row `lifecycle_state` equals `workspace.lifecycle_state`
- row `workflow_preset_id` equals `workspace.placement_plan.selected_workflow_preset.id`

Alternative considered: keep validation only around insert. This does not protect reads after manual corruption, interrupted future migrations, or future update bugs.

### Keep row consistency failure as `workspace_catalog_unavailable`

The existing command error model already treats SQLite read, migration, and decoding failures as `workspace_catalog_unavailable`. Row/payload mismatch is the same class of problem: the catalog cannot safely return authoritative durable state.

Alternative considered: introduce a more specific command error. That would expand the frontend contract without a clear v1 recovery path.

### Keep JSON as the returned Workspace representation

The read path should still return the decoded `workspace_json` after consistency validation. This avoids rebuilding nested Workspace objects from partial columns and keeps the schema lightweight for v1.

Alternative considered: make columns authoritative and reconstruct Workspaces from normalized tables. That is more robust long term, but too large for this targeted consistency change.

## Risks / Trade-offs

- Existing inconsistent local data would become unreadable -> acceptable for v1 because returning inconsistent state as authoritative is more dangerous than surfacing catalog unavailability.
- More columns are selected on reads -> negligible performance impact for the expected local catalog size.
- Future indexed columns could be added without validation -> mitigate with repository tests and a helper that centralizes row-to-Workspace decoding and consistency checks.

## Migration Plan

No schema migration is required.

Implementation can be rolled back by returning to JSON-only reads. Any consistent catalog rows remain compatible in both directions.

## Open Questions

- Should `created_at` and `updated_at` eventually become part of the Workspace contract or remain repository metadata only? Default for this change: leave timestamps out of consistency validation because they are not duplicated in `Workspace`.
