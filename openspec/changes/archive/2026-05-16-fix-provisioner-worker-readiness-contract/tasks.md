## 1. Native Worker Contract Alignment

- [x] 1.1 Update the Provisioner Worker start request contract to send the worker's `job_id` field while preserving the selected Workflow Preset payload.
- [x] 1.2 Update the Provisioner Worker status response parser to accept nullable phases for idle, succeeded, failed, and cancelled terminal states.
- [x] 1.3 Parse current worker response fields including `diagnostic_message` and structured `error` metadata without exposing raw payloads or secrets.
- [x] 1.4 Map current worker statuses and phases into existing Workspace Provisioning progress phases without leaking worker-specific phases into durable domain state.

## 2. Readiness and Error Classification

- [x] 2.1 Distinguish worker transport/readiness failures from unrecoverable worker API contract failures in the worker gateway or service boundary.
- [x] 2.2 Return running provisioning progress when a running Provisioning Pod has a temporarily unreachable worker and the same pod remains safe to retry.
- [x] 2.3 Ensure worker validation errors, malformed successful payloads, unsupported statuses, and invalid progress percentages are not classified as worker unavailability.
- [x] 2.4 Persist sanitized worker terminal failure details, including stable worker error code or reason metadata when available.
- [x] 2.5 Treat temporary non-JSON proxy/readiness responses as worker unavailability, while preserving worker JSON validation and contract errors as response-invalid failures.

## 3. Regression Tests

- [x] 3.1 Add parser tests for idle worker status with `phase: null`, terminal success with no phase, terminal failure diagnostics, and current worker phase vocabulary.
- [x] 3.2 Add gateway/service tests proving running pod plus temporarily unreachable worker returns running progress and does not mark the Workspace failed.
- [x] 3.3 Add service tests proving idle worker status triggers `POST /start` with `job_id` and selected Workflow Preset.
- [x] 3.4 Add tests proving worker validation or malformed payload failures are classified distinctly from worker readiness/unavailability.
- [x] 3.5 Add or update failure-detail tests for sanitized worker `diagnostic_message`, `code`, and `reason_code` propagation.
- [x] 3.6 Add classification tests for non-JSON proxy readiness responses versus worker JSON contract errors.

## 4. Generated Contracts and UI Surface

- [x] 4.1 Regenerate TypeScript command bindings if Rust response or failure-detail types change.
- [x] 4.2 Update frontend error/progress presentation only if generated contract changes require a UI adjustment.
- [x] 4.3 Confirm React continues to consume Native-owned provisioning state without locally classifying worker-specific raw responses.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Run `bun run build` if generated frontend bindings or frontend code changed.
- [x] 5.5 Run `bun run lint --fix` if generated frontend bindings or frontend code changed.
- [x] 5.6 Create a fresh workspace provisioning attempt and verify the flow waits through worker readiness, starts the worker job, and surfaces any real worker preparation failure with structured diagnostics.
