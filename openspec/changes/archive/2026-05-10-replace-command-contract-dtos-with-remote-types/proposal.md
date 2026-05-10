## Why

Command contracts currently duplicate domain models solely to derive generated TypeScript bindings. The duplication is largest in `src-tauri/src/commands/workspace/contracts.rs`, but the same pattern exists across command modules and increases the chance that generated DTOs drift from the domain shapes they are meant to expose.

## What Changes

- Replace duplicated command DTO graphs with command-owned Specta remote type exports for domain models.
- Keep `specta::Type` and generated binding concerns owned by the Tauri command boundary.
- Continue to keep domain modules independent from command handlers, Tauri runtime APIs, provider transport DTOs, and generated frontend binding derives.
- Preserve existing command names, UI-safe errors, and secret-handling behavior.
- **BREAKING**: Change Workspace Setup command payloads that contain `PlacementPlan` to use the provider-discriminated domain shape, adding the nested `gpu_cloud_provider_id` discriminator.
- Keep command-specific request and response wrapper structs where they define the command API boundary.
- Remove manual `From` mappings that only convert between identical command DTO and domain model shapes.
- Rely on generated `src/generated/commands.ts` diffs during review for generated TypeScript contract shape changes.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `native-boundaries`: Clarify that command-owned generated binding concerns may be implemented through command-boundary Specta remote type exports for domain models, without deriving generated binding traits in domain modules, and that intentional command contract migrations may expose domain-discriminated shapes directly.
- `gpu-cloud-provider-setup`: Clarify that Provider Setup command payload compatibility must be preserved while avoiding duplicated setup snapshot DTOs where remote generated binding metadata is sufficient.
- `workspace-setup`: Change Workspace Setup command payloads to expose provider-discriminated domain Placement Plan and Workspace shapes directly while removing duplicated workspace command DTO graphs.

## Impact

- Affected native modules:
  - `src-tauri/src/commands/*`
  - `src-tauri/src/commands/provider_setup/*`
  - `src-tauri/src/commands/workspace/*`
  - `src-tauri/src/commands/contracts.rs`
  - `src-tauri/src/domain/*` only if needed to make remote type exports compile without adding generated binding derives
  - `src-tauri/src/provider_setup/*` where command mapping call sites simplify
  - `src-tauri/src/workspace_setup/*` where command mapping call sites simplify
- Generated bindings:
  - `src/generated/commands.ts` will change for Workspace Setup `PlacementPlan` and nested Workspace placement payloads.
- Verification:
  - Run `cargo test`
  - Run `cargo clippy --fix --allow-dirty --allow-staged`
  - Run `cargo fmt`
  - Run `bun run build`
  - Run `bun run lint --fix`
