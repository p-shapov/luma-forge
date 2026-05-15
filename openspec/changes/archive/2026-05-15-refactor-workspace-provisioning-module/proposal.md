## Why

`src-tauri/src/workspace_provisioning/mod.rs` has grown into a 1000-line module that mixes gateway contracts, lifecycle coordination, provisioning orchestration, progress derivation, snapshot mapping, and helpers. The behavior is safety-critical and already acts like a durable state machine, so keeping the responsibilities entangled makes future provisioning changes harder to review without accidentally changing idempotency, cleanup metadata, or secret-handling guarantees.

## What Changes

- Split the workspace provisioning implementation into focused Rust modules while preserving the existing public application behavior and command contract.
- Extract provider and worker gateway contracts, provisioning result/input/output DTOs, per-workspace coordination, progress derivation, snapshot mapping helpers, and the application service into separate files under `src-tauri/src/workspace_provisioning/`.
- Reduce the size and responsibility of `workspace_provisioning/mod.rs` so it primarily declares submodules and re-exports the public provisioning API.
- Decompose the large `WorkspaceProvisioningService::sync` workflow into smaller step-oriented helpers that preserve the current "at most one safe action per sync call" semantics.
- Keep existing tests meaningful, updating module paths and adding focused tests only where extraction exposes previously implicit behavior.
- No frontend command, generated binding, database schema, provider API, or user-visible behavior changes are intended.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `workspace-provisioning`: add explicit preservation requirements for the modular refactor so the implementation split keeps the existing provisioning command contract, one-action sync semantics, durable persistence guarantees, and secret-safety behavior unchanged.

## Impact

- Affected native code:
  - `src-tauri/src/workspace_provisioning/mod.rs`
  - `src-tauri/src/workspace_provisioning/tests.rs`
  - Potential new files under `src-tauri/src/workspace_provisioning/`
  - Import sites such as `src-tauri/src/provider/registry.rs`, `src-tauri/src/workspace_resource_cleanup/mod.rs`, and `src-tauri/src/app_state.rs` if re-export paths require adjustment.
- No new dependencies are expected.
- No database migration is expected.
- Verification should include `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt` for `src-tauri/` changes.
