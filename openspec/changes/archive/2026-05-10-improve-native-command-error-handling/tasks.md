## 1. Command Error Contract

- [x] 1.1 Expand `NativeCommandErrorCode` with UI-safe codes for request validation, placement validation, catalog/profile reads, provider availability/authorization/response failures, stored-key invalid state, provider setup deletion missing state, and Workspace Catalog storage/migration/corruption categories.
- [x] 1.2 Add optional UI-safe metadata to `NativeCommandError` for field, reason category, or recovery action without exposing source error strings or sensitive payloads.
- [x] 1.3 Update command error mapping tests to cover every Provider Setup and Workspace Setup error variant.
- [x] 1.4 Regenerate TypeScript command bindings and update spec reference native contracts to match the generated error contract.

## 2. Provider Setup Errors

- [x] 2.1 Split submitted Provider API Key validation into required/blank and provider-unauthorized categories.
- [x] 2.2 Preserve stored Provider API Key parse failures as a distinct stored-key invalid category through Provider Setup and Workspace Setup prerequisite checks.
- [x] 2.3 Split RunPod identity failures into provider availability, unauthorized key, and invalid identity response categories.
- [x] 2.4 Change delete-missing setup behavior to map to `provider_setup_not_found` while preserving successful corrupt setup deletion behavior.
- [x] 2.5 Add or update Provider Setup service, provider registry, RunPod client, and command mapping tests for the new categories.

## 3. Workspace Setup Read Errors

- [x] 3.1 Split bundled catalog reader failures so Workflow Catalog, Provisioning Profiles, and Endpoint Profiles each map to source-specific UI-safe codes.
- [x] 3.2 Split Provider Inventory lookup failures into missing setup, stored key invalid, unauthorized key, provider availability, provider response invalid, and provider inventory invalid categories.
- [x] 3.3 Add or update tests for catalog/profile read commands and Provider Inventory failure mappings.

## 4. Workspace Creation Errors

- [x] 4.1 Split Workspace creation request validation into invalid Workspace UUID, missing Workspace name, and invalid Workspace metadata categories.
- [x] 4.2 Replace broad Placement Plan validation collapse with typed validation reasons for provider mismatch, missing datacenter, missing GPU, stale Workflow Preset, stale Provisioning Profile, stale Endpoint Profile, incompatible endpoint profile, and insufficient storage size.
- [x] 4.3 Map typed placement validation reasons into UI-safe command errors with optional field/recovery metadata.
- [x] 4.4 Add or update Workspace Setup service tests for each request and placement validation category.

## 5. Workspace Catalog Errors

- [x] 5.1 Coordinate with `refine-workspace-catalog-error-handling` so Workspace Catalog internal errors remain repository-owned while command mapping exposes safe storage, migration, query, corruption, and schema mismatch categories.
- [x] 5.2 Update Workspace Catalog initialization, list, and insert error mappings to preserve duplicate Workspace behavior and expose safe recovery categories for non-duplicate failures.
- [x] 5.3 Add or update Workspace Catalog and command mapping tests for storage unavailable, migration failed, query failed, corrupt data, row mismatch, and duplicate Workspace UUID.

## 6. Frontend Error Presentation

- [x] 6.1 Add a frontend native-command error presenter keyed by `NativeCommandErrorCode` and optional metadata.
- [x] 6.2 Update the command console to show actionable error copy and recovery hints while preserving raw JSON as secondary development detail if still useful.
- [x] 6.3 Add frontend coverage or focused type checks for exhaustive command error handling if the project has test/type-check support available.

## 7. Verification

- [x] 7.1 Run `cargo test`.
- [x] 7.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 7.3 Run `cargo fmt`.
- [x] 7.4 Run `bun run build`.
- [x] 7.5 Run `bun run lint --fix`.
