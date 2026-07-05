# Task 5 Report

## What I implemented

- Updated `src-tauri/src/infra/bundled/repositories/runtime_presets.rs` to derive `BundledRuntimePreset.id` and `.revision` from the asset path instead of DTO fields removed from generated types.
- Updated `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs` to derive `BundledRuntimeContract.id` and `.revision` from the asset path.
- Updated `src-tauri/src/infra/bundled/repositories/execution_schemas.rs` to derive `BundledExecutionSchema.id` and `.revision` from the asset path.
- Updated `src-tauri/src/infra/bundled/repositories/workflows.rs` so `BundledWorkflow.id` and `.revision` come from repository path parameters instead of workflow metadata payload fields.
- Added path identity helpers in each touched repository file per brief.
- Kept repository lookup misses returning `Ok(None)`.
- Kept parse and assembly failures mapped to `BundledCatalogError::CorruptBundledAsset`.
- Added focused unit tests in each touched repository module for path-derived identity and `get(...)=Ok(None)` miss behavior.
- Tightened workflow revision enumeration so malformed workflow metadata paths surface `CorruptBundledAsset` instead of being ignored.

## What I tested and results

- `cargo fmt --manifest-path src-tauri/Cargo.toml` -> passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` -> passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` -> blocked by pre-existing legacy include errors outside Task 5 scope.

Exact blocked output:

```text
error: couldn't read `src/runtime_catalog/../../../bundled/runtime-contracts.json`: No such file or directory (os error 2)
 --> src/runtime_catalog/bundled.rs:7:38

error: couldn't read `src/workflow_catalog/../../../bundled/workflow-catalog.json`: No such file or directory (os error 2)
  --> src/workflow_catalog/bundled.rs:12:37

error: couldn't read `src/workflow_catalog/../../../bundled/execution-schemas.json`: No such file or directory (os error 2)
  --> src/workflow_catalog/bundled.rs:13:38
```

## TDD / RED-GREEN evidence

### RED

Before the repository changes, `cargo test --manifest-path src-tauri/Cargo.toml runtime_presets -- --nocapture` failed with:

```text
error[E0609]: no field `id` on type `generated::ExecutionSchema`
error[E0609]: no field `revision` on type `generated::ExecutionSchema`
error[E0609]: no field `id` on type `generated::RuntimeContract`
error[E0609]: no field `revision` on type `generated::RuntimeContract`
error[E0609]: no field `id` on type `RuntimePreset`
error[E0609]: no field `revision` on type `RuntimePreset`
error[E0609]: no field `id` on type `WorkflowMetadata`
error[E0609]: no field `revision` on type `WorkflowMetadata`
```

This was the expected failure mode from the brief: generated DTOs no longer expose top-level identity.

### GREEN

- Added parser/repository unit tests that assert path-derived identity and `Ok(None)` misses in the four touched repository files.
- After implementation, the DTO-field compile errors no longer appear in `cargo test`; the remaining failure is only the known legacy `include_str!` blocker in old catalog modules.
- Formatting verification passed after the code change.

## Files changed

- `src-tauri/src/infra/bundled/repositories/runtime_presets.rs`
- `src-tauri/src/infra/bundled/repositories/runtime_contracts.rs`
- `src-tauri/src/infra/bundled/repositories/execution_schemas.rs`
- `src-tauri/src/infra/bundled/repositories/workflows.rs`
- `.superpowers/sdd/task-5-report.md`

## Self-review findings

- No scope drift: changes are limited to the four repository files plus the required report.
- No legacy compatibility paths added.
- Workflow list path parsing now fails closed on malformed metadata paths, which is consistent with the brief’s `CorruptBundledAsset` requirement.
- Residual verification gap: native tests cannot complete until the out-of-scope legacy include paths are fixed or removed.

## Issues or concerns

- `cargo test --manifest-path src-tauri/Cargo.toml` is still blocked by legacy catalog modules referencing removed bundled registry JSON files:
  - `bundled/runtime-contracts.json`
  - `bundled/workflow-catalog.json`
  - `bundled/execution-schemas.json`
- I did not modify those modules because the task brief explicitly marked that path as potential out-of-scope blocker.
