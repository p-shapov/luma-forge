# Final Fix Report

## Scope

Addressed bundled catalog review findings in the active `src-tauri` iteration without adding compatibility shims or wiring old aggregate readers back into runtime paths.

## Changes Made

1. `src-tauri/src/infra/bundled/validation.rs`
   - Added a path-derived expected schema check before JSON Schema validation.
   - Rejects `$schema` values that do not match the approved entity kind implied by the bundled path.
   - Added a fixture test for a mismatched `metadata.json` path using the model-assets schema.
   - Tightened `validation_rejects_unsafe_model_asset_paths` to assert the exact path and message.
2. `src-tauri/build.rs`
   - Made direct schema file iteration deterministic by sorting immediate schema paths before loading.
3. `src-tauri/src/infra/bundled/mod.rs`
   - Hid the validation module from runtime public exports by compiling it only for tests.
4. `src-tauri/src/infra/bundled/generated.rs`
   - Stopped compiling generated bundled types into the runtime module; kept only the bundled manifest include.
5. `src-tauri/Cargo.toml`
   - Moved `jsonschema` out of normal runtime dependencies into `dev-dependencies`.
   - Removed direct `regress` dependency after generated bundled types stopped compiling into runtime.
6. `src-tauri/Cargo.lock`
   - Updated for the dependency manifest change above.

## Commands Run

1. `cargo fmt --manifest-path src-tauri/Cargo.toml`
   - Exit 0.
2. `cargo test --manifest-path src-tauri/Cargo.toml infra::bundled::validation`
   - Exit 101.
   - Blocked by legacy aggregate `include_str!` readers:
     - `src/runtime_catalog/bundled.rs:7` missing `../../../bundled/runtime-contracts.json`
     - `src/workflow_catalog/bundled.rs:12` missing `../../../bundled/workflow-catalog.json`
     - `src/workflow_catalog/bundled.rs:13` missing `../../../bundled/execution-schemas.json`
3. `cargo test --manifest-path src-tauri/Cargo.toml`
   - Exit 101.
   - Same three `include_str!` blockers as above.
4. `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
   - Exit 0.
5. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
   - Exit 101.
   - Same three `include_str!` blockers as above.
6. `cargo tree --manifest-path src-tauri/Cargo.toml | rg "regress|jsonschema"`
   - Output showed `jsonschema` in the graph and only transitive `regress v0.11.1`.
   - No direct `regress v0.10.5` dependency remained after the runtime generated-type include was removed.
7. `git diff --check HEAD^ HEAD`
   - Exit 0.
   - No whitespace or conflict-marker issues in the committed diff.

## Intentionally Unresolved Finding

Finding 2 remains accepted unresolved branch state by instruction:

- `src-tauri/src/runtime_catalog/bundled.rs`
- `src-tauri/src/workflow_catalog/bundled.rs`

These legacy readers still reference removed aggregate files through `include_str!`. They were left untouched to avoid adding compatibility shims or routing through `old_bundled`. They currently block `cargo test` and `cargo clippy` for the crate until that legacy reader cleanup lands in a later iteration.

## Summary

- Fixed the critical schema/path trust issue at the shared validation entry point.
- Hid build-time validation from runtime exports.
- Moved validation-only runtime baggage out of normal dependencies and removed the direct `regress` dependency.
- Verification is partially blocked by the known legacy aggregate reader compile errors described above.
