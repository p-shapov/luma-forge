## Why

Provisioning and endpoint profiles no longer represent real product choices: v1 uses one standardized Provisioner Worker and one standardized Endpoint Worker. Keeping profiles as selectable catalog objects adds stale-profile validation, duplicated runtime fields, and generated frontend surface area without improving the current provisioning model.

## What Changes

- **BREAKING** Remove Provisioning Profiles and Endpoint Profiles as domain, catalog, command, and frontend contract concepts.
- **BREAKING** Remove profile read commands and generated TypeScript bindings for `get_provisioning_profiles`, `get_endpoint_profiles`, `ProvisioningProfile`, and `EndpointProfile`.
- **BREAKING** Remove selected provisioning and endpoint profiles from `PlacementPlan` and Workspace snapshots.
- Keep Workflow Presets as the selectable catalog unit for Workspace Setup.
- Resolve standardized provisioning and endpoint worker references from build-time Native configuration instead of reading profile catalogs when provisioning consumes them.
- Parse required worker image ref and port build environment variables as non-empty strings during the Tauri native build; missing or blank values fail the build.
- Remove fixed provider values from profile/catalog contracts and defer introducing provider constants until provisioning implementation needs them.
- Remove the current legacy Workspace JSON compatibility migration because pre-production Workspace data compatibility is no longer required.

## Capabilities

### New Capabilities

- `native-build-configuration`: Defines build-time parsing of Native worker image refs and ports and exposes them to native code through Cargo build environment output.

### Modified Capabilities

- `workspace-setup`: Removes profile catalog reads, profile selection, profile compatibility validation, and legacy profile migration requirements from Workspace Setup.
- `native-boundaries`: Removes provider-specific profile configuration as domain data and updates command boundary expectations after profiles are removed.

## Impact

- Affects bundled catalog files, domain profile/placement/workspace models, Workspace Setup services, bundled catalog parsing/validation, Workspace Catalog persistence migrations, generated command contracts, and React setup screens.
- Existing local dev Workspace Catalog data may become invalid and may need to be cleared manually.
- No production migration compatibility is required for this change.
