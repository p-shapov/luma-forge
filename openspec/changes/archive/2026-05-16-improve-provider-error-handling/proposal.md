## Why

Provider failures currently collapse into broad UI-safe errors before the Native Layer has enough context to choose the right recovery path. In practice this can make a provider request rejection look like provider unavailability, which gives the Client poor retry guidance.

The app should not depend on RunPod error codes or message strings. RunPod-specific response interpretation should stay inside the RunPod provider adapter and be mapped into stable LumaForge-owned provider errors.

## What Changes

- Add stable provider error variants for rate limiting and provider request rejection.
- Map RunPod REST status classes into LumaForge-owned provider errors without exposing RunPod-specific codes downstream.
- Map RunPod GraphQL identity and inventory errors into the same provider error variants where applicable.
- Map provider errors into Provider Setup, Workspace Setup, and Workspace Provisioning use-case errors.
- Add stable native command error codes, reasons, retryability, and recovery actions for provider rate limiting and provider request rejection.
- Keep command responses, generated bindings, frontend presentation, and native command logs UI-safe and secret-free.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `native-boundaries`: Provider-local errors need stable LumaForge-owned variants for rate limiting and request rejection without leaking provider transport details.
- `workspace-provisioning`: Provider rate limiting and provider request rejection need distinct command-facing recovery semantics.
- `workspace-setup`: Provider inventory failures need clearer classification for rate limiting and non-authentication request rejection.
- `native-command-logging`: Command logs must continue to include only stable UI-safe command metadata and must not include provider payloads or secrets.

## Impact

- Affects `src-tauri/src/provider`, especially the RunPod adapter and provider registry mappings.
- Affects Workspace Setup and Workspace Provisioning error enums and mapping tests.
- Affects command error mapping, generated TypeScript bindings, and frontend error presentation copy.
- Affects native command logging tests to ensure provider request rejection remains secret-free.
- Does not add provider diagnostic payloads, operation-context error wrappers, lifecycle policy changes, a hosted backend, local ML execution, or dependency on RunPod-specific error codes as domain contracts.
