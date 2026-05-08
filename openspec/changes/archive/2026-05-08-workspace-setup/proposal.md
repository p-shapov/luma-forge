## Why

LumaForge needs a native-owned Workspace Setup boundary before provisioning can safely create remote resources. The current Rust layer can validate provider setup, but it does not yet expose workflow/profile catalogs, provider placement inventory, or durable `Draft` Workspace creation.

## What Changes

- Add native commands to read the bundled Workflow Catalog, Provisioning Profiles, and Endpoint Profiles as separate read-only calls.
- Add a native command to read the local Workspace Catalog from SQLite.
- Add a native command to fetch RunPod placement inventory after validating provider setup and keyring access.
- Add a native command to create one `Draft` Workspace from a client-generated UUID, name, provider id, and full selected Placement Plan objects.
- Store user-owned Workspace Catalog state in SQLite, while keeping bundled workflow/profile definitions as read-only app-owned catalog data.
- Persist selected Workflow Preset, Provisioning Profile, and Endpoint Profile objects into the Workspace record as creation-time snapshots.
- Validate catalog/profile compatibility and placement structure before persistence, but do not validate live GPU availability during Workspace creation.
- Reject duplicate Workspace UUID creation attempts with `workspace_already_exists`.
- Keep provider resource creation, provisioning jobs, cleanup, and full React Workspace Setup UI outside this change.

## Capabilities

### New Capabilities

- `workspace-setup`: Native-owned catalog reading, provider placement lookup, Workspace Catalog reading, and durable `Draft` Workspace creation for the Workspace Setup flow.

### Modified Capabilities

None.

## Impact

- Affected native code: Tauri command boundary, generated TypeScript command bindings, workspace setup application service, domain models for workflows/profiles/placement/workspaces, bundled catalog loading, SQLite-backed Workspace Catalog repository, RunPod provider inventory adapter, native error mapping.
- Affected frontend contract: new generated commands and response types for workflow catalog, provisioning profiles, endpoint profiles, provider inventory, workspace catalog, and workspace creation.
- Affected local state: introduces SQLite persistence for the Workspace Catalog; Provider API Keys remain stored only in the secure keyring.
- Affected external system: RunPod GraphQL/API inventory lookup is called for placement options; Workspace creation itself does not create, modify, attach, or delete provider resources.
