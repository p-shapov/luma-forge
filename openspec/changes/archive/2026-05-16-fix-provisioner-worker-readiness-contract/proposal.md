## Why

Workspace provisioning currently surfaces `provisioner_worker_unavailable` while a RunPod provisioning pod is still starting and can also fail permanently before preparation starts because Native rejects the Provisioner Worker's valid idle status payload. This blocks real provisioning diagnostics and makes normal provider readiness lag look like a user-facing error.

## What Changes

- Treat a running provisioning pod with an unreachable worker endpoint as a non-terminal worker readiness wait state instead of a command error or durable workspace failure.
- Align Native's Provisioner Worker status parser with the current worker API, including `phase: null` for idle and terminal statuses, current worker phase names, diagnostic fields, and terminal cancellation semantics.
- Align Native's `POST /start` request with the worker contract by sending the expected job correlation identifier and selected Workflow Preset payload.
- Reclassify worker API errors so startup/transport failures remain retryable readiness conditions while worker validation, malformed payload, authorization, conflict, and terminal job failures produce distinct UI-safe classifications.
- Preserve sanitized worker failure diagnostics from `diagnostic_message` and structured worker `error` metadata without exposing secrets.
- Add regression coverage for readiness lag, idle status parsing, start request shape, current worker phase mapping, worker validation errors, and sanitized terminal failure details.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-provisioning`: Clarify how Native handles Provisioner Worker readiness lag, worker status parsing, start request shape, worker progress mapping, and durable worker failure classification.
- `provisioner-worker`: Clarify the worker API status/start contract consumed by Native, including idle/terminal phase nullability, job identifier field, phase vocabulary, and UI-safe diagnostic metadata.

## Impact

- Affects the Tauri workspace provisioning service, Provisioner Worker HTTP gateway/parser, workspace provisioning error/failure mapping, generated command bindings if error or failure types change, and related Rust tests.
- May affect Python worker tests or docs only if the worker contract needs minor clarification rather than Native-only adaptation.
- Does not add hosted backend dependencies, local ML execution, provider API key exposure, or frontend-owned durable provisioning decisions.
