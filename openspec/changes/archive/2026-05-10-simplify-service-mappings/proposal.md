## Why

Native application services currently sit behind an extra layer of application DTOs that mostly duplicate domain models and require repeated mapping before and after service logic. This makes service code harder to read, obscures domain invariants, and spreads ownership of native serialization across too many modules.

## What Changes

- Make native domain models the canonical serializable shape for local catalogs, workspace persistence, and application service inputs/results.
- Keep generated frontend binding concerns isolated to Tauri command DTOs; domain types MUST NOT derive `specta::Type`.
- Remove the redundant workspace application contract layer represented by `workspace_contracts.rs`.
- Remove service-facing setup DTOs where services can accept domain values or simple service input structs directly.
- Move profile and placement modeling from generic provider-parameterized structs to provider-discriminated domain unions.
- Model RunPod provisioning profiles, endpoint profiles, and placement plans as RunPod-specific domain variants instead of generic domain profiles carrying provider-owned config generics.
- Organize validation by domain concept with domain-owned validators such as `placement_validator`, `profiles_validator`, and `provider_inventory_validator`; keep bundled parsers/readers responsible for bundled resource loading, deserialization, and error adaptation only.
- Keep provider HTTP/GraphQL request and response DTOs inside provider implementation modules.
- Preserve existing command payload compatibility, UI-safe error behavior, secret handling, catalog validation, and SQLite row consistency checks.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `native-boundaries`: Clarify that domain models may derive native serialization traits for catalog and persistence boundaries, while generated binding traits remain command-owned.
- `workspace-setup`: Require Workspace Setup services, repositories, catalog readers, profiles, and placement validation to use domain-native types directly.
- `gpu-cloud-provider-setup`: Require Provider Setup services to use domain-native inputs/results directly instead of service-facing setup DTOs where no command contract is needed.

## Impact

- Affected native modules:
  - `src-tauri/src/domain/*`
  - `src-tauri/src/workspace/*`
  - `src-tauri/src/provider_setup/*`
  - `src-tauri/src/commands/workspace/*`
  - `src-tauri/src/commands/provider_setup/*`
  - `src-tauri/src/bundled/*`
  - `src-tauri/src/provider/runpod/*`
- Command DTOs remain the generated frontend contract and continue to map to/from domain types.
- SQLite workspace JSON and bundled catalog JSON will deserialize directly into domain-owned serializable types.
- Bundled catalog parsing remains in `bundled`, while domain invariant validation is delegated to domain-owned validator modules grouped by validated type or aggregate.
- No React command names, generated command payload semantics, or UI-safe error codes are intended to change.
- No provider secrets may be added to domain snapshots, command payloads, persistence records, logs, or diagnostics.
