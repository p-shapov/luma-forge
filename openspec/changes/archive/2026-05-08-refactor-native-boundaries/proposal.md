## Why

The native layer now contains multiple completed flows and is about to grow into workspace provisioning, but several implementation boundaries still couple provider clients, command response errors, and application services. Cleaning those boundaries now reduces the risk that provisioning duplicates error models, persistence APIs, and provider-specific logic across flows.

## What Changes

- Introduce an explicit native boundary capability that defines where command contracts, application services, provider clients, durable workspace persistence, and bundled contracts live.
- Move command-safe error DTOs out of provider setup code and map use-case errors at the command boundary.
- Introduce provider-local client errors so provider implementations do not depend on setup or workspace use-case errors.
- Keep provider-specific HTTP, GraphQL DTOs, and mapping code inside provider modules while exposing provider-agnostic gateway implementations to application services.
- Fix workspace catalog persistence to derive stored provider identifiers from workspace data instead of hardcoding the v1 provider.
- Split large setup modules into directories with separate contract, service, and test files where doing so reduces responsibility mixing.
- Preserve generated command contracts and user-visible behavior for GPU cloud provider setup and workspace setup.

## Capabilities

### New Capabilities

- `native-boundaries`: Defines native-layer module boundaries, dependency direction, provider error isolation, command error mapping, and workspace persistence responsibilities.

### Modified Capabilities

- None.

## Impact

- Affected native modules: `src-tauri/src/commands`, `src-tauri/src/provider`, `src-tauri/src/provider_setup.rs`, `src-tauri/src/workspace`, and `src-tauri/src/secrets.rs`.
- Affected generated frontend contract only if Specta type paths or exported command error ownership change; command request/response shapes should remain behaviorally compatible.
- No new runtime dependencies are expected.
- Verification remains the native-layer verification set: `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt`; run frontend build/lint only if generated TypeScript changes.
