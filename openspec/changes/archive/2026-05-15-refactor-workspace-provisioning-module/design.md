## Context

Workspace provisioning is implemented primarily in `src-tauri/src/workspace_provisioning/mod.rs`. The file currently contains gateway traits, provider input/observation DTOs, worker gateway adaptation, per-workspace concurrency coordination, the application service, the full sync workflow, progress derivation, snapshot helpers, and error mapping helpers.

The sync workflow is safety-critical because it derives the next provisioning action from durable Workspace metadata, performs at most one safe action per sync call, persists provider resource snapshots before later dependencies use them, and preserves cleanup metadata on failure. The current single-file shape makes those boundaries harder to see during review.

This change should be treated as a behavior-preserving native refactor. The existing frontend command contract, generated TypeScript bindings, provider interactions, database schema, and OpenSpec requirements remain intact.

## Goals / Non-Goals

**Goals:**

- Split workspace provisioning into focused Rust modules with clear responsibility boundaries.
- Keep the existing public provisioning API available through `crate::workspace_provisioning` re-exports where practical.
- Preserve the current one-action-per-sync behavior, durable persistence order, cancellation cleanup policy, and secret-safety guarantees.
- Make `WorkspaceProvisioningService::sync` easier to review by extracting step-oriented helpers.
- Keep verification centered on the existing provisioning test suite, adding focused tests only where extraction makes implicit behavior easier to test directly.

**Non-Goals:**

- Do not change workspace provisioning behavior, command response shapes, generated frontend bindings, provider API requests, database schemas, or user-visible progress semantics.
- Do not introduce new dependencies such as `async-trait`.
- Do not implement a generic provider-resource lifecycle engine.
- Do not repair failed workspaces or change resource discovery/idempotency behavior beyond the existing implementation.

## Decisions

### Split by responsibility, not by provider resource alone

Create focused modules under `src-tauri/src/workspace_provisioning/`:

- `contracts.rs`: provisioning input/output DTOs and `WorkspaceProvisioningResult`
- `gateways.rs`: `ProviderProvisioningGateway`, `ProvisionerWorkerGateway`, and the HTTP worker adapter
- `coordinator.rs`: `WorkspaceProvisioningCoordinator` and its RAII guard
- `progress.rs`: `progress_for_workspace` and result assembly
- `snapshots.rs`: provider observation to Workspace snapshot helpers and readiness/status helpers
- `service.rs`: `WorkspaceProvisioningConfig`, `WorkspaceProvisioningService`, and workflow methods
- `mod.rs`: module declarations and public re-exports

Alternative considered: split only by resource (`volume.rs`, `pod.rs`, `template.rs`, `endpoint.rs`). That maps to the workflow phases but would scatter shared concerns such as persistence and one-action return semantics. Responsibility-based modules keep contracts, progress, coordination, and orchestration boundaries explicit.

### Keep the service as the orchestration owner

`WorkspaceProvisioningService` should remain the application-layer entry point for `initiate`, `sync`, and `cancel`. The refactor should not move workflow decisions into Tauri command handlers, provider registry code, or domain models.

Alternative considered: create a separate state machine executor type immediately. That may become useful later, but doing it in the same change would mix module extraction with a more substantial design rewrite. This proposal keeps the first refactor conservative.

### Extract sync into step-oriented helpers

Keep the observable sync loop unchanged while decomposing the large method into helpers shaped around durable workflow activities:

- synchronize persistent storage volume
- create or observe provisioning pod
- drive provisioner worker preparation
- delete completed provisioning pod and token
- create or observe endpoint template
- create or observe serverless endpoint
- mark ready when required snapshots are ready

Each helper should either return a completed `WorkspaceProvisioningResult` for the single action it performed, or indicate that the next helper may be considered. This keeps the "at most one safe action per sync call" rule visible.

Alternative considered: build a fully explicit `ProvisioningAction` planner now. That would make pure decision tests attractive, but it is a larger behavioral refactor. The implementation can leave room for a future planner without requiring it for this change.

### Preserve existing async trait style

The project currently uses explicit `Pin<Box<dyn Future<...>>>` gateway traits. This refactor should keep that style and avoid adding `async-trait`, because this is a structural cleanup rather than a dependency or trait-model change.

Alternative considered: migrate gateway traits to `async-trait`. That could reduce boilerplate, but it would add a dependency and alter the trait implementation style across related modules without being necessary for the simplification goal.

### Re-export stable API names from `workspace_provisioning`

Existing import sites should continue to use `crate::workspace_provisioning::{...}` where possible. Internal modules may use `pub(crate)` helpers, but the public surface needed by `provider::registry`, `workspace_resource_cleanup`, `app_state`, and tests should remain discoverable from the module root.

Alternative considered: require all callers to import from new submodules. That would expose internal organization throughout the crate and make future rearrangement noisier.

## Risks / Trade-offs

- Module extraction can accidentally change visibility or import paths -> keep root re-exports for existing public names and verify all native tests compile.
- Step helper extraction can accidentally allow more than one action per sync call -> structure helpers around early return semantics and keep existing sync behavior tests intact.
- Snapshot helper extraction can hide persistence ordering mistakes -> keep provider observation to snapshot conversion pure, while catalog updates remain in the service layer.
- Moving progress derivation can drift from workflow state checks -> keep progress tests and compare behavior through existing command/service tests.
- Over-splitting can make the workflow harder to follow -> keep `service.rs` as the readable orchestration spine and avoid generic lifecycle abstractions in this change.

## Migration Plan

1. Add the new module files and move existing types/helpers into them without changing behavior.
2. Update `mod.rs` to declare modules and re-export the public provisioning API.
3. Move the service implementation into `service.rs`.
4. Extract sync internals into focused private helper methods while preserving one-action early returns.
5. Update tests and import paths.
6. Run native verification: `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, and `cargo fmt`.

Rollback is straightforward because this change does not migrate data or external contracts: revert the Rust module extraction if verification reveals behavioral drift.
