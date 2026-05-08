## Why

Secret storage and shared provider command DTOs currently depend on the Provider Setup module even when they are consumed by Workspace Setup. This keeps one application flow acting as shared infrastructure and makes future native flows more likely to inherit the same coupling.

## What Changes

- Introduce a secret-storage-owned error type for secure keyring and stored-secret decoding failures.
- Map secret storage failures independently into Provider Setup and Workspace Setup use-case errors.
- Remove the Workspace Setup dependency on Provider Setup errors for secret access.
- Move shared provider command DTOs, starting with `GpuCloudProviderId`, into a neutral native contract module that can be imported by Provider Setup, Workspace Setup, and persistence code.
- Preserve current user-visible command behavior, generated provider id values, error codes, and retryability semantics.
- Defer the broader service/command contract split to a separate architectural cleanup.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-boundaries`: Clarifies ownership for secret storage errors and shared provider command DTOs across native modules.

## Impact

- Affected native modules: `src-tauri/src/secrets.rs`, `src-tauri/src/provider_setup`, `src-tauri/src/workspace`, and a new neutral shared contract module under `src-tauri/src`.
- Affected tests: provider setup, workspace setup, provider client, workspace catalog, command error mapping, and generated binding export tests.
- Generated TypeScript command shapes should remain behaviorally compatible; `GpuCloudProviderId` should still export as the same `"runpod"` union.
- No persistence migration or runtime dependency changes are expected.
