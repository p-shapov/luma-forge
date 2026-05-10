## 1. Contract Baseline

- [x] 1.1 Capture the current generated TypeScript command shapes for Provider Setup and Workspace Setup as a comparison baseline.
- [x] 1.2 Identify every command DTO that only mirrors a domain model shape and every command wrapper that should remain command-owned.
- [x] 1.3 Confirm `#[specta(remote = ...)]` works with the project’s Specta version for structs, string enums, internally tagged enums, and nested domain fields.

## 2. Shared Command Types

- [x] 2.1 Add command-boundary remote generated binding metadata for the domain `GpuCloudProviderId` while preserving the generated `"runpod"` TypeScript union.
- [x] 2.2 Update shared command contract usage so command wrappers can use the domain provider id where payload shape is unchanged.
- [x] 2.3 Remove provider id mapping code that only translates between identical command and domain provider id values.

## 3. Provider Setup Commands

- [x] 3.1 Add command-boundary remote generated binding metadata for the domain `GpuCloudProviderSetup`.
- [x] 3.2 Update Provider Setup response wrappers to expose optional or required domain setup snapshots without a duplicated setup DTO.
- [x] 3.3 Preserve `SetupGpuCloudProviderRequest` as a command-owned request wrapper with redacted `Debug` output.
- [x] 3.4 Remove mechanical Provider Setup setup-snapshot DTO mapping while preserving command error and secret redaction behavior.

## 4. Workspace Setup Commands

- [x] 4.1 Add command-boundary remote generated binding metadata for workflow catalog domain types.
- [x] 4.2 Add command-boundary remote generated binding metadata for provisioning and endpoint profile domain types.
- [x] 4.3 Change the Workspace Setup command contract to use the domain provider-discriminated Placement Plan shape directly.
- [x] 4.4 Add command-boundary remote generated binding metadata for provider inventory domain types.
- [x] 4.5 Add command-boundary remote generated binding metadata for workspace catalog, workspace, and workspace resource snapshot domain types.
- [x] 4.6 Update Workspace Setup command wrappers to use domain-native nested payloads where serialized shape is unchanged.
- [x] 4.7 Remove mechanical Workspace Setup command DTOs and `From` implementations that only map between identical command and domain shapes.

## 5. Compatibility Checks

- [x] 5.1 Keep the native binding export test passing for Provider Setup generated types.
- [x] 5.2 Keep the native binding export test passing for Workspace Setup generated types.
- [x] 5.3 Add serialization tests for representative tagged enums used by command payloads, including provider-discriminated and source-discriminated shapes.
- [x] 5.4 Regenerate `src/generated/commands.ts` and review the diff for unintended command name, field, discriminant, or error-shape changes.

## 6. Verification

- [x] 6.1 Run `cargo test`.
- [x] 6.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 6.3 Run `cargo fmt`.
- [x] 6.4 Run `bun run build`.
- [x] 6.5 Run `bun run lint --fix`.
- [x] 6.6 Run `openspec validate replace-command-contract-dtos-with-remote-types --strict`.
