## 1. Native Domain and Contracts

- [x] 1.1 Add Rust domain models for Workflow Catalog, Workflow Presets, Provisioning Profiles, Endpoint Profiles, Provider Inventory, Placement Plan, Workspace lifecycle state, Workspace snapshots, and Workspace Catalog.
- [x] 1.2 Add Specta/Tauri request and response types for `get_workflow_catalog`, `get_provisioning_profiles`, `get_endpoint_profiles`, `get_provider_inventory`, `get_workspace_catalog`, and `create_workspace`.
- [x] 1.3 Extend native command error mapping for Workspace Setup storage, catalog, placement, provider setup, provider API, and duplicate Workspace UUID failures.
- [x] 1.4 Ensure generated frontend types expose only UI-safe data and never include Provider API Keys.

## 2. Bundled Catalog Loading

- [x] 2.1 Add bundled resource files for the initial Workflow Catalog, Provisioning Profiles, and Endpoint Profiles.
- [x] 2.2 Implement read-only catalog loading for bundled workflow/profile definitions.
- [x] 2.3 Validate bundled catalogs for non-empty ids, provider compatibility, workflow execution type compatibility, and internally consistent required fields.
- [x] 2.4 Add native service methods and commands that return Workflow Catalog, Provisioning Profiles, and Endpoint Profiles separately.
- [x] 2.5 Add tests for successful catalog reads and unavailable, empty, malformed, or inconsistent bundled catalog failures.

## 3. Provider Inventory Lookup

- [x] 3.1 Add a provider inventory gateway capability to the provider registry without broadening the existing identity-only setup contract.
- [x] 3.2 Implement RunPod inventory lookup and map provider-specific datacenter/GPU data into UI-safe Provider Inventory DTOs.
- [x] 3.3 Validate stored Provider API Key presence before inventory lookup and reject missing setup before any Provider call.
- [x] 3.4 Map RunPod timeout, transport, status, GraphQL, and response parsing failures to `provider_api_unavailable`.
- [x] 3.5 Add tests for inventory success, missing setup, provider failure, and secret redaction.

## 4. SQLite Workspace Catalog

- [x] 4.1 Add SQLite initialization and migration support for the Workspace Catalog.
- [x] 4.2 Create a Workspace Catalog schema with indexed Workspace id, provider id, lifecycle state, workflow preset id, timestamps, and serialized Workspace payload.
- [x] 4.3 Implement a Workspace Catalog repository for listing Workspaces, finding by id, and inserting a new Workspace transactionally.
- [x] 4.4 Ensure duplicate Workspace UUID insert attempts return `workspace_already_exists` without mutating the existing record.
- [x] 4.5 Add repository tests for empty catalog, list, create, duplicate id, write failure, decode failure, and re-read after insert.

## 5. Workspace Creation Service

- [x] 5.1 Implement the Workspace Setup application service that coordinates catalog readers, the secret store, provider registry, and Workspace Catalog repository.
- [x] 5.2 Validate Workspace UUID, non-empty Workspace name, provider id, provider setup/keyring presence, full Placement Plan completeness, storage minimum, and profile/provider/workflow compatibility.
- [x] 5.3 Validate submitted full Workflow Preset, Provisioning Profile, and Endpoint Profile objects against bundled canonical definitions before persistence.
- [x] 5.4 Persist created Workspaces with lifecycle state `draft` and empty Provider Resource snapshots.
- [x] 5.5 Re-read the persisted Workspace after insert and return only that authoritative record to the command boundary.
- [x] 5.6 Ensure Workspace creation does not validate live GPU/datacenter availability and does not create, modify, attach, or delete Provider Resources.
- [x] 5.7 Add service tests for successful creation, duplicate Workspace UUID, missing provider setup, invalid Placement Plan, stale submitted catalog objects, incompatible profiles, insufficient storage, and persistence failure.

## 6. Command Wiring and Bindings

- [x] 6.1 Wire Workspace Setup service methods into Tauri commands while keeping command handlers thin.
- [x] 6.2 Register new commands in the Tauri Specta command builder.
- [x] 6.3 Regenerate TypeScript command bindings and verify the frontend contract includes all new Workspace Setup commands.
- [x] 6.4 Keep existing GPU Cloud Provider Setup commands and behavior unchanged.

## 7. Verification

- [x] 7.1 Run `cargo test` for native changes.
- [x] 7.2 Run `cargo clippy --fix --allow-dirty --allow-staged` for native changes.
- [x] 7.3 Run `cargo fmt` for native changes.
- [x] 7.4 Run `bun run build` after generated frontend bindings change.
- [x] 7.5 Run `bun run lint --fix` after generated frontend bindings change.
