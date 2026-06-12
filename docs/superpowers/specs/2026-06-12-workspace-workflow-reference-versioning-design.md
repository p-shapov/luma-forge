# Workspace Workflow Reference Versioning Design

## Summary

Persist workspaces with a workflow catalog reference instead of embedding the full workflow preset snapshot. Workflow versioning belongs to the workflow catalog shape: a `WorkflowPreset` owns its revision history, and application services resolve the persisted workflow reference against the bundled workflow catalog when they need executable workflow details.

This keeps workspace persistence small and stable while making workflow versioning explicit in the catalog model.

## Current State

`src-tauri/src/domain/workspace.rs` currently stores a full `WorkflowPreset` in `Workspace`.

`src-tauri/src/workspace_catalog/sqlite.rs` persists that value as `workflow_preset_json`.

`create_workspace` currently accepts a `workflow_preset_id`, resolves it from `WorkflowCatalogService`, and passes the full preset into provisioned-remote workspace creation.

Provisioning later resolves endpoint and provisioner contracts from the `WorkflowPreset` embedded in the workspace.

## Target Domain Model

Add a small workflow reference value:

```rust
pub struct WorkflowReference {
    pub id: String,
    pub version: String,
}
```

Change `Workspace` to store that reference:

```rust
pub struct Workspace {
    pub id: String,
    pub workflow: WorkflowReference,
    pub state: WorkspaceState,
    pub runtime: WorkspaceRuntime,
}
```

`Workspace` remains the persisted aggregate owned by `workspace_catalog`. Workflow metadata, revisions, model assets, and runtime requirements remain owned by `workflow_catalog`.

Change the workflow catalog shape so versioning is part of `WorkflowPreset`:

```rust
pub struct WorkflowPreset {
    pub id: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub revisions: Vec<WorkflowRevision>,
}

pub struct WorkflowRevision {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}
```

`WorkflowReference.version` selects a revision inside the referenced preset. `WorkflowPreset.revisions` must contain unique `WorkflowRevision.version` values. `WorkflowRevision` carries revision-specific executable requirements such as Hugging Face key requirements, remote runtime requirements, and required model assets.

## Persistence

Replace `workflow_preset_json TEXT NOT NULL` with explicit columns:

```sql
workflow_id TEXT NOT NULL,
workflow_version TEXT NOT NULL
```

Because LumaForge is pre-v1, do not add a legacy compatibility path for the old `workflow_preset_json` schema unless explicitly requested. The bootstrap should validate the current schema directly.

## Catalog Resolution

Catalog resolution should happen at application boundaries, not inside the repository.

The repository should read and write persisted workspace data only. It should not know how to load bundled workflow catalogs.

Callers that need executable workflow details should resolve the preset from the catalog, then resolve the referenced revision through `WorkflowPreset`:

```rust
impl WorkflowPreset {
    pub fn resolve_revision(
        &self,
        reference: &WorkflowReference,
    ) -> Option<&WorkflowRevision>;
}
```

`resolve_revision` must return `None` unless `self.id == reference.id` and exactly one revision has `version == reference.version`.

## Create Workspace Flow

`create_workspace` should accept `workflow_preset_id` and `workflow_revision_version`.

The command loads the workflow catalog, finds the requested preset by id, and resolves the requested revision by version. Workspace creation validates the selected placement against that revision, then persists only:

- `workspace.id`
- `workspace.workflow.id`
- `workspace.workflow.version`
- `workspace.state`
- `workspace.runtime`

The workspace must not persist the full preset or revision payload.

## Provisioning Flow

Provisioning must resolve the full `WorkflowPreset` from `workspace.workflow` before selecting endpoint and provisioner contracts.

Resolution must require the workflow id to match exactly. If no matching preset exists, provisioning fails explicitly. After resolving the preset, provisioning must resolve the referenced revision by matching `WorkflowRevision.version == Workspace.workflow.version`.

The existing contract resolver should stop reading `workspace.workflow_preset` and should receive the already resolved `WorkflowRevision`.

Passing the resolved revision into the contract resolver keeps catalog lookup separate from runtime contract selection.

## Workspace Reads and Events

The persisted domain workspace should expose `workflow: WorkflowReference`.

Command response DTOs may keep the current `workflowPreset` shape for frontend stability by resolving each workspace reference before conversion. That preserves the frontend-facing contract while keeping persistence normalized.

If a workspace references a missing workflow preset during a read, the command should return an explicit native error rather than silently omitting the workspace or substituting a different preset.

Do not mutate `WorkspaceState` only because the bundled catalog no longer contains the referenced workflow. Lifecycle state remains persisted runtime state; catalog compatibility is a derived read or operation failure.

## Versioning Rules

Workflow reference identity is:

- `id`
- `version`

`WorkflowReference.id` identifies the preset. `WorkflowReference.version` identifies the revision inside that preset.

Workflow revision identity is the pair of the owning preset id and `WorkflowRevision.version`. Changing runtime requirements, required model assets, base volume size, or Hugging Face key requirements must create a new workflow revision.

Existing workspaces keep their workflow reference. They do not store a full preset snapshot.

Changing preset-level metadata such as name or execution type can update the preset in place unless the catalog needs to preserve the older metadata as a separate preset id.

## Error Behavior

Missing workflow reference:

- create flow: return an explicit command error before creating a workspace.
- workspace read flow: return an explicit command error.
- provision flow: fail before creating remote resources.

Duplicate workflow preset ids in the workflow catalog remain invalid catalog data. Duplicate revision versions inside a preset are also invalid catalog data.

## Testing

Update native tests for:

- `Workspace` serialization stores `workflow` reference, not an embedded preset.
- SQLite schema validates `workflow_id` and `workflow_version`.
- SQLite insert, update, list, and find round-trip workflow references.
- create workspace resolves and persists the requested preset id and revision version exactly.
- provisioning resolves the full preset by workflow reference and resolves its referenced revision.
- missing workflow reference fails explicitly.
- workflow catalog validation rejects duplicate preset ids and duplicate revision versions inside a preset.

## Out of Scope

- Frontend behavior changes.
- Workspace workflow upgrade command.
- Legacy migration from `workflow_preset_json`.
- Persisting full workflow preset snapshots.
