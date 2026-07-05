Final review fix report

- Finding: `src-tauri/src/infra/bundled/validation.rs` cross-file validation classified typed assets by path instead of `asset.schema_id`.
- Files changed:
  - `src-tauri/src/infra/bundled/validation.rs`

Commands and results

- `cargo test --manifest-path src-tauri/Cargo.toml validation_rejects_runtime_contract_disguised_as_runtime_preset_path -- --exact`
  - Result: failed before tests ran due to existing compile baseline errors:
    - `src/runtime_catalog/bundled.rs`: missing `../../../bundled/runtime-contracts.json`
    - `src/workflow_catalog/bundled.rs`: missing `../../../bundled/workflow-catalog.json`
    - `src/workflow_catalog/bundled.rs`: missing `../../../bundled/execution-schemas.json`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - Result: initially failed on formatting in the new test.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - Result: passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - Result: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml validation_rejects_runtime_contract_disguised_as_runtime_preset_path -- --exact`
  - Result: same existing compile baseline errors as above; focused test still could not run.

Fix summary

- Added a focused cross-file test for a runtime-contract-shaped asset stored under `runtime_presets/base/1.0.0.json`.
- Updated typed reference set construction to classify `runtime_presets`, `runtime_contracts`, `execution_schemas`, and `execution_schema_inputs` from `asset.schema_id` instead of path bucket alone.
- Kept approved path checks and path identity validation separate.
