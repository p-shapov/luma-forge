# Task 3 Report

## What Changed

- Added `src-tauri/src/infra/bundled/validation.rs` with:
  - `BundledAsset`
  - `BundledValidationError`
  - `SchemaDocument`
  - path/secret helper predicates
  - `validate_bundled_catalog`
  - `validate_cross_file_assets`
  - fixture tests from the brief
- Added `src-tauri/src/infra/bundled/mod.rs`.
- Exposed `infra::bundled` from `src-tauri/src/infra/mod.rs`.
- Replaced `src-tauri/build.rs` with build-time helpers that:
  - watch `../bundled` and `schemas/bundled`
  - load bundled schemas
  - generate `OUT_DIR/bundled_types.rs`
  - validate bundled assets through `validation.rs`
  - generate `OUT_DIR/bundled_manifest.rs`
- Updated `src-tauri/Cargo.toml` build dependencies for the new build script and added `jsonschema` to runtime dependencies because `validation.rs` is compiled both by the build script and the main crate.
- `src-tauri/Cargo.lock` changed from dependency resolution.

## TDD Evidence

1. Wrote `src-tauri/src/infra/bundled/validation.rs` tests first.
2. Ran the brief’s command before module wiring:
   - `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation::tests`
   - Result: failed before reaching the new tests because existing catalog modules still reference removed bundled files.
3. Wired `infra::bundled`, replaced `build.rs`, and reran the same command.
   - Result: same external blocker remained after Task 3 changes.

## Commands And Results

- `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation::tests`
  - First run result: failed.
  - Exact failure reason:
    - `src/runtime_catalog/bundled.rs` tries to `include_str!("../../../bundled/runtime-contracts.json")`
    - `src/workflow_catalog/bundled.rs` tries to `include_str!("../../../bundled/workflow-catalog.json")`
    - `src/workflow_catalog/bundled.rs` tries to `include_str!("../../../bundled/execution-schemas.json")`
    - all three files no longer exist after the bundled JSON reshape from Tasks 1-2
- `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation::tests`
  - Second run result: failed with the same three missing-file errors after the Task 3 wiring/build-script fixes compiled past the new code path.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - Result: passed.
- `git diff --check HEAD^ HEAD`
  - Result: passed.
- `git commit -m "feat(bundled): add build validation gate"`
  - Result: passed.

## Files Changed

- `src-tauri/Cargo.lock`
- `src-tauri/Cargo.toml`
- `src-tauri/build.rs`
- `src-tauri/src/infra/bundled/mod.rs`
- `src-tauri/src/infra/bundled/validation.rs`
- `src-tauri/src/infra/mod.rs`

## Self-Review Findings

- Task 3 scope is in place: tests/module wiring/build-time generation/validation skeleton were added and committed.
- The remaining compile failure is outside the new Task 3 files and comes from pre-existing catalog readers that still expect the old bundled aggregate JSON files.
- I did not change those catalog readers because that would start Task 4/5 work.

## Concerns

- The required cargo test command does not pass yet because the crate still contains stale `include_str!` references to removed bundled assets:
  - `bundled/runtime-contracts.json`
  - `bundled/workflow-catalog.json`
  - `bundled/execution-schemas.json`
- Because the crate stops there, Task 3’s generated `OUT_DIR` artifacts are not end-to-end verified through a successful full test run in this iteration.

## Task 3 Review Fixes

### What Changed

- Updated `src-tauri/src/infra/bundled/validation.rs` so `validate_bundled_catalog(...) -> Result<...>` now returns `BundledValidationError::Invalid { path, message }` for per-asset read, parse, missing `$schema`, unknown schema, validator construction, and schema validation failures.
- Kept the build gate behavior in `src-tauri/build.rs` unchanged: it still unwraps the `Result` and can panic the build when validation fails.
- Removed the `#[cfg(not(test))]` gates around `SchemaDocument`, `validate_bundled_catalog`, and the file walkers so the validation path is testable.
- Added stdlib-only fixture tests for:
  - one valid tiny bundled entity
  - one corruption case proving invalid JSON returns `Err` instead of panicking

### Commands Run

- `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation::tests`
  - Result: failed during crate compilation before test execution.
  - Output summary: unchanged legacy `include_str!` paths are missing:
    - `src/runtime_catalog/../../../bundled/runtime-contracts.json`
    - `src/workflow_catalog/../../../bundled/workflow-catalog.json`
    - `src/workflow_catalog/../../../bundled/execution-schemas.json`
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infra::bundled::validation::tests`
  - Result: failed for the same three missing-file `include_str!` paths.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - Result: passed.

### Output Summary

- The validation module change is in place and formatted.
- The required focused test command is still blocked by unrelated legacy bundled-catalog compile errors outside the touched files.

## Task 3 Re-Review Fix

### What Changed

- Updated `src-tauri/src/infra/bundled/validation.rs` so bundled catalog traversal fails closed:
  - missing or unreadable root directories now return `BundledValidationError::Invalid { path, message }`
  - directory entry read failures now return the same error instead of panicking
- Updated `src-tauri/build.rs` so schema/bundled directory traversal panics instead of silently skipping unreadable directories.
- Added a small stdlib-only test that checks a missing bundled root returns `Err` without depending on bundled catalog contents.

### Commands Run

- `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - Result: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation::tests`
  - Result: failed before executing the targeted tests.
  - Output summary: unchanged legacy `include_str!` paths are still missing:
    - `src/runtime_catalog/../../../bundled/runtime-contracts.json`
    - `src/workflow_catalog/../../../bundled/workflow-catalog.json`
    - `src/workflow_catalog/../../../bundled/execution-schemas.json`

### Output Summary

- The traversal path now fails closed in validation and panics in the build script on unreadable directories.
- The requested focused test command is still blocked by pre-existing legacy bundled-catalog `include_str!` failures outside the touched files.
