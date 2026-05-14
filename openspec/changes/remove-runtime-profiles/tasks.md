## 1. Native Build Configuration

- [x] 1.1 Add build-time Native app configuration that reads Provisioner Worker image ref, Provisioner Worker port, Endpoint Worker image ref, and Endpoint Worker port from build env with `.env` fallback.
- [x] 1.2 Parse required worker config values as non-empty strings during the Tauri native build and fail the build on missing or blank values.
- [x] 1.3 Remove fixed RunPod runtime values from profile/catalog contracts and defer provider constants until provisioning code consumes them.
- [x] 1.4 Keep runtime startup free of worker config validation.

## 2. Remove Profile Domain And Catalogs

- [x] 2.1 Remove Provisioning Profile and Endpoint Profile domain types, validators, test fixtures, and command remote type metadata.
- [x] 2.2 Remove `resources/catalog/provisioning-profiles.json` and `resources/catalog/endpoint-profiles.json` plus bundled catalog parser/reader code for profile catalogs.
- [x] 2.3 Update bundled catalog validation so Workspace Setup reads and validates only the Workflow Catalog.
- [x] 2.4 Remove profile catalog read commands and command error paths for `get_provisioning_profiles` and `get_endpoint_profiles`.
- [x] 2.5 Remove generated TypeScript bindings and frontend command access for profile commands and profile types.

## 3. Simplify Placement And Workspace Models

- [x] 3.1 Remove selected Provisioning Profile and selected Endpoint Profile fields from `PlacementPlan`.
- [x] 3.2 Update Workspace domain construction, validation, persistence serialization, and row consistency checks for profile-free Placement Plans.
- [x] 3.3 Update Workspace Setup validation to compare only the selected Workflow Preset against bundled Workflow Catalog data.
- [x] 3.4 Remove profile provider-mismatch, stale-profile, and endpoint-profile compatibility validation branches and tests.
- [x] 3.5 Remove the existing legacy Workspace JSON compatibility migration and related migration tests.

## 4. Update React Workspace Setup

- [x] 4.1 Remove profile fetching, profile state, profile selection, and profile-loading UI paths from Workspace Setup screens/stores.
- [x] 4.2 Update Placement Plan creation in React to submit only provider placement choices and the selected Workflow Preset.
- [x] 4.3 Update frontend tests or type-level expectations affected by regenerated command bindings.

## 5. Update Documentation And Reference Contracts

- [x] 5.1 Update `spec/reference` entity and native contract files to remove Provisioning Profile and Endpoint Profile contracts.
- [x] 5.2 Update workspace setup and workspace provisioning flow docs to describe Native-owned build-time worker configuration instead of selected profiles.
- [x] 5.3 Update ubiquitous language docs and domain index links to remove profile concepts or mark them replaced by Native build-time configuration.
- [x] 5.4 Document required worker build environment variables for development.

## 6. Verification

- [x] 6.1 Regenerate TypeScript command bindings.
- [x] 6.2 Run `cargo test`.
- [x] 6.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 6.4 Run `cargo fmt`.
- [x] 6.5 Run `bun run build`.
- [x] 6.6 Run `bun run lint --fix`.
- [x] 6.7 Run `openspec status --change remove-runtime-profiles` and confirm the change is apply-ready.
