## Context

LumaForge is a macOS-only Tauri application where React owns presentation and temporary UI state, while the Rust native layer owns durable state, provider access, secure storage, and authoritative workflow validation. The native layer currently implements GPU Cloud Provider Setup for RunPod API key validation and keyring storage, but Workspace Setup still exists only as flow documentation and reference TypeScript sketches.

Workspace Setup is the first native flow that introduces user-owned durable state. It must bridge read-only bundled catalog definitions, live RunPod placement inventory, and a SQLite-backed Workspace Catalog without creating provider resources. The resulting `Draft` Workspace is the durable input to later Workspace Provisioning.

## Goals / Non-Goals

**Goals:**

- Expose separate native read commands for Workflow Catalog, Provisioning Profiles, Endpoint Profiles, Provider Inventory, and Workspace Catalog.
- Add native Workspace creation that persists one complete `Draft` Workspace into SQLite.
- Keep bundled Workflow Presets, Provisioning Profiles, and Endpoint Profiles as read-only app-owned definitions.
- Accept full selected Workflow Preset, Provisioning Profile, and Endpoint Profile objects in the create request as part of `PlacementPlan`.
- Validate full submitted catalog/profile objects against bundled definitions before persistence.
- Persist the selected Workflow Preset, Provisioning Profile, and Endpoint Profile objects as Workspace creation-time snapshots.
- Validate provider setup and keyring access before provider inventory lookup or Workspace creation.
- Reject duplicate Workspace UUID creation attempts with `workspace_already_exists`.
- Keep Native-owned state authoritative and return re-read persisted Workspace records after creation.

**Non-Goals:**

- No full React Workspace Setup UI.
- No provider resource creation, attachment, deletion, provisioning job, or cleanup workflow.
- No live GPU availability validation during Workspace creation.
- No support for GPU cloud providers other than RunPod in v1.
- No Provider API Key persistence outside the secure keyring.
- No user editing of bundled Workflow Presets, Provisioning Profiles, or Endpoint Profiles.

## Decisions

### Store Canonical Catalogs as Bundled Read-Only Resources

Workflow Presets, Provisioning Profiles, and Endpoint Profiles are app-owned definitions shipped with the application, not user-owned records. Native catalog loaders should read bundled resource files and expose them through separate commands.

Alternative considered: store catalog/profile definitions primarily in SQLite. This was rejected because these definitions are versioned with the app and are not user-created in v1.

### Store Workspace Catalog in SQLite

Workspace Catalog entries are durable user-owned state and should be persisted in SQLite through a repository abstraction. The first schema can keep a small set of indexed columns plus a serialized Workspace payload:

```text
workspaces
  id                  uuid/text primary key
  name                text
  gpu_cloud_provider_id text
  lifecycle_state      text
  workflow_preset_id   text
  created_at           timestamp
  updated_at           timestamp
  workspace_json        json/text
```

This gives idempotent lookup, duplicate detection, transaction boundaries, and a low-friction path for future provisioning metadata.

Alternative considered: a filesystem JSON Workspace Catalog. This was rejected because the project already includes SQLx/SQLite and provisioning will need transactional updates.

### Accept Full Placement Objects but Validate Against Bundled Definitions

The create request should accept the full `PlacementPlan` object, including selected Workflow Preset, Provisioning Profile, and Endpoint Profile. Native validation still treats bundled definitions as authoritative:

- submitted object ids must exist in the bundled catalogs;
- submitted objects must match the canonical bundled definitions for those ids;
- provider ids must match the requested GPU Cloud Provider;
- endpoint profile execution type must match the selected workflow preset execution type;
- requested volume size must satisfy the workflow preset minimum.

The persisted Workspace stores the selected objects as creation-time snapshots. Existing Workspaces therefore retain the exact preset/profile payload they were created with even if bundled definitions change in a future app version.

Alternative considered: accept only ids and scalar placement fields. This would reduce request payload size but would not reflect the current product direction to pass full selected objects through the contract.

### Keep Setup Read Commands Separate

Native commands should remain separate for Workflow Catalog, Provisioning Profiles, Endpoint Profiles, Workspace Catalog, and Provider Inventory. This keeps command responses focused, avoids forcing every setup screen refresh to re-fetch all data, and matches the existing reference contract shape.

Alternative considered: one combined setup-data command. This was rejected because independent commands better support targeted retry and clearer error handling.

### Use Provider Inventory Only for Placement Options

Provider inventory lookup validates provider setup and the stored Provider API Key before calling RunPod. It returns datacenters, GPU options per datacenter, and provider maximum persistent storage volume size when known. The lookup is read-only with respect to the Workspace Catalog and provider resources.

Workspace creation validates structural placement consistency, but does not revalidate live GPU availability. Provider inventory may change between selection and creation, and final availability belongs to the provisioning flow.

Alternative considered: require selected GPU/datacenter to still be available during create. This was rejected to keep Workspace Setup a metadata creation flow and avoid treating volatile inventory as a durable invariant.

### Treat Duplicate Workspace UUID as an Error

If `create_workspace` receives a Workspace UUID that already exists in SQLite, it returns `workspace_already_exists` and does not return the existing record as success.

Alternative considered: make create idempotent by UUID. This was rejected in favor of surfacing duplicate creation attempts clearly to the Client.

### Keep Command Handlers Thin

Tauri command handlers should only deserialize request types, call application services, and map typed errors into `NativeCommandError`. Workflow decisions belong in a Workspace Setup service, with side effects behind catalog, provider, secret-store, and repository traits.

The intended dependency direction is:

```text
Commands
  -> WorkspaceSetupService
       -> BundledCatalogReader
       -> WorkspaceCatalogRepository
       -> SecretStore
       -> ProviderRegistry
            -> ProviderInventoryGateway
```

This follows the existing provider setup service pattern and avoids coupling domain logic to Tauri runtime APIs.

## Risks / Trade-offs

- Bundled catalog schema drift could break existing Workspace snapshots -> Store selected objects as snapshots and validate new creates against the current bundled catalog only.
- JSON-heavy Workspace persistence may defer some relational constraints -> Keep indexed identity/state/provider columns and move fields to normalized tables only when provisioning needs it.
- Full-object create requests can become stale or tampered with -> Validate submitted objects against bundled canonical definitions before persistence.
- Provider inventory is volatile -> Treat inventory lookup as advisory placement data and leave availability enforcement to provisioning.
- SQLite initialization failures block Workspace Setup -> Map them to `workspace_catalog_unavailable` or `local_storage_unavailable` with retryable metadata where appropriate.
- RunPod inventory API shape may differ from current sketches -> Isolate provider-specific parsing in the RunPod adapter and expose only UI-safe provider inventory DTOs.

## Migration Plan

No existing Workspace Catalog user data needs migration because native Workspace persistence does not currently exist.

Implementation should introduce SQLite initialization and migrations before exposing Workspace creation. If initialization fails, Workspace Setup commands fail without mutating provider resources or keyring state.

Rollback before release can remove the new commands and SQLite migration because there is no supported user data yet. After release, rollback must preserve the Workspace Catalog database file even if the downgraded build ignores it.

## Open Questions

None.
