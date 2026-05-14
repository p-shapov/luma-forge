## Context

LumaForge currently exposes Workflow Presets, Provisioning Profiles, and Endpoint Profiles during Workspace Setup. Provisioning and endpoint profiles were introduced to describe worker images, ports, worker paths, mount paths, provider config, and compatibility between a selected Workflow Preset and worker runtimes.

The current product direction is narrower: v1 has one standardized Provisioner Worker contract and one standardized Endpoint Worker contract. The user-facing setup choice is the Workflow Preset plus provider placement details. Provisioning and endpoint worker details are implementation configuration owned by the Native Layer, not independent selectable product concepts.

This change is pre-production and does not need to preserve existing local Workspace Catalog data.

## Goals / Non-Goals

**Goals:**

- Remove Provisioning Profiles and Endpoint Profiles from catalogs, domain models, placement plans, command contracts, frontend state, and persistence snapshots.
- Keep Workflow Presets as the only bundled catalog data selected during Workspace Setup.
- Resolve standardized worker references from build-time Native configuration when provisioning consumes them.
- Parse required worker image refs and ports during the native build so an incorrectly configured build fails before producing a binary.
- Remove profile-driven provider values from Workspace Setup contracts without adding unused provider constants yet.
- Remove the legacy Workspace JSON migration that only exists for unstable pre-production compatibility.

**Non-Goals:**

- No Workspace Provisioning implementation is added by this change.
- No support for multiple provisioner implementations, endpoint worker variants, providers, or per-workflow runtime images is introduced.
- No production data migration compatibility is provided for existing dev Workspace records.
- No new React UX for generation or provisioning is added.

## Decisions

### Remove profiles rather than collapsing them into hidden profiles

Provisioning and Endpoint Profiles will be deleted as domain concepts instead of being made internal-only catalogs. Keeping hidden profiles would preserve the same stale-object validation and duplicated runtime fields while only removing the frontend surface. Native should instead own one build-time configuration path for the standardized workers.

Alternative considered: keep profiles but stop exposing them to React. This was rejected because profiles still imply multiple selectable runtime contracts and require compatibility validation that v1 does not need.

### Keep Workflow Preset snapshots in placement

Workspace Setup will continue to validate and persist the selected Workflow Preset as the catalog-backed user/product selection. The Workflow Preset contains real provisioning inputs: ComfyUI source, model assets, custom nodes, install paths, execution type, and storage minimum.

Alternative considered: submit only `workflow_preset_id`. This was rejected for this change because the current domain already validates complete selected catalog objects and persists creation-time snapshots. Removing profiles is enough to reduce the unnecessary abstraction without changing the Workflow Preset snapshot strategy.

### Parse worker build config during native build

The native build script will read required worker image refs and worker ports from the build environment, falling back to the project `.env` file for development. Each required value is parsed as a trimmed non-empty string and emitted through `cargo:rustc-env` so native code can consume it at compile time. Missing or blank values fail the build before a native binary is produced.

Alternative considered: validate during application startup. This was rejected because these values are build configuration; a missing required value should fail the native build rather than produce a binary that crashes at launch.

### Defer fixed provider values until provisioning implementation

RunPod-specific values that are not product choices, such as secure cloud type, workspace mount path, and container disk size, will not be represented in profiles or Workspace Setup contracts. This change removes those profile fields now and defers introducing provider constants until the provisioning implementation actually consumes them.

Alternative considered: add provider constants in this change. This was deferred because no provisioning code consumes them yet, and adding unused constants would recreate placeholder configuration surface.

### Remove the legacy Workspace JSON migration

The existing compatibility migration for old Workspace JSON will be removed. The migration mechanism may remain, but this specific migration is not needed before production and would complicate a deliberate breaking schema cleanup.

Alternative considered: write a migration from profile-bearing Workspaces to profile-free Workspaces. This was rejected because the app is pre-production, the only known data is developer-local, and supporting old profile snapshots would preserve complexity that this change is intentionally removing.

## Risks / Trade-offs

- Existing local dev Workspace data may fail to load -> Clear the local dev Workspace Catalog after applying the change.
- Build-time env parsing can make local builds stricter -> Document required worker image ref and port variables and provide development defaults in `.env.example`.
- Removing profiles reduces future runtime flexibility -> Reintroduce a runtime selection abstraction only when there is a concrete second worker/provider runtime to select.
- Build-time Cargo environment output means changing `.env` requires rebuilding the native app -> Document this behavior for development.

## Migration Plan

This is a breaking pre-production change. Apply it directly, regenerate command bindings, and update React to the new Workspace Setup contract. Existing local dev Workspace data may be cleared manually.

Rollback is reverting the change before production data depends on the profile-free Workspace schema.

## Open Questions

- None.
