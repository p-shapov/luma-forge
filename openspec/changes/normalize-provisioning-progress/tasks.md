## 1. Native Progress Semantics

- [x] 1.1 Add native helpers/constants for global Workspace Provisioning progress anchors.
- [x] 1.2 Update Workspace-derived progress so active provider phases return normalized total percentages.
- [x] 1.3 Keep `cleaning_up` and `failed` progress outside the ready-progress percentage scale.

## 2. Provisioning Worker Outcome Mapping

- [x] 2.1 Map worker readiness lag and worker start responses without active preparation work to `starting_provisioning_pod` at `10%` in Workspace Provisioning.
- [x] 2.2 Map worker-local preparation progress into the global `40..90%` range in Workspace Provisioning.
- [x] 2.3 Use the `preparing_environment` lower bound when a running worker preparation status has no percent.
- [x] 2.4 Preserve existing worker failure, cancellation, and secret-safety behavior.

## 3. Frontend Contract Usage

- [x] 3.1 Confirm React progress rendering uses the Native-provided `percent` without duplicating phase math.
- [x] 3.2 Regenerate generated command bindings if the exported Native contract output changes.

## 4. Tests and Verification

- [x] 4.1 Add or update Rust tests for Workspace-derived phase anchors.
- [x] 4.2 Add or update Rust tests for worker readiness lag, idle/start handling, preparation scaling, missing worker percent, cancellation, and failure progress.
- [x] 4.3 Run `cargo test`.
- [x] 4.4 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.5 Run `cargo fmt`.
- [x] 4.6 If generated bindings or frontend files change, run `bun run build` and `bun run lint --fix`.
