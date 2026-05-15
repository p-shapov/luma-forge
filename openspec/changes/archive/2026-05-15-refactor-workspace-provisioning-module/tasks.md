## 1. Module Structure

- [x] 1.1 Create focused `workspace_provisioning` submodules for contracts, gateways, coordinator, progress, snapshots, and service.
- [x] 1.2 Update `workspace_provisioning/mod.rs` to declare submodules and re-export the public provisioning API used by existing callers.
- [x] 1.3 Move provider input DTOs, provider observations, and `WorkspaceProvisioningResult` into `contracts.rs` without changing their fields.
- [x] 1.4 Move provider and worker gateway traits plus the HTTP worker gateway adapter into `gateways.rs`.
- [x] 1.5 Move `WorkspaceProvisioningCoordinator` and its RAII guard into `coordinator.rs`.

## 2. Service Refactor

- [x] 2.1 Move `WorkspaceProvisioningConfig` and `WorkspaceProvisioningService` into `service.rs` while preserving the existing `new`, `initiate`, `sync`, and `cancel` API.
- [x] 2.2 Extract progress result assembly and `progress_for_workspace` into `progress.rs`.
- [x] 2.3 Extract provider observation to Workspace snapshot mapping, RunPod template snapshot access, readiness checks, and terminal status checks into `snapshots.rs`.
- [x] 2.4 Keep catalog access, durable Workspace mutation, and provisioning orchestration owned by `WorkspaceProvisioningService`.

## 3. Sync Decomposition

- [x] 3.1 Extract the network volume create/refresh branch into focused private service helpers that preserve existing persistence and failure behavior.
- [x] 3.2 Extract provisioning pod create/observe behavior into focused private service helpers that preserve worker token handling and provider status behavior.
- [x] 3.3 Extract Provisioner Worker polling/start/success handling into a focused private service helper that preserves worker progress responses and terminal failure handling.
- [x] 3.4 Extract completed provisioning pod deletion and worker-token cleanup into a focused private service helper.
- [x] 3.5 Extract endpoint template create/refresh behavior into focused private service helpers.
- [x] 3.6 Extract serverless endpoint create/refresh behavior and ready-state completion into focused private service helpers.
- [x] 3.7 Ensure each extracted sync helper either performs one safe action and returns, or explicitly allows the next helper to be considered without mutating state.

## 4. Call Sites and Tests

- [x] 4.1 Update native import paths in provider registry, workspace resource cleanup, app state, and tests while keeping root re-exports stable where practical.
- [x] 4.2 Update workspace provisioning tests to compile against the new module structure without weakening existing behavior assertions.
- [x] 4.3 Add focused tests for extracted pure helpers only if they clarify readiness, terminal status, or progress derivation behavior.
- [x] 4.4 Confirm no frontend command contracts, generated binding names, database schemas, or provider request payloads changed.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Review the final diff to confirm the change is structural and does not alter provisioning semantics.
