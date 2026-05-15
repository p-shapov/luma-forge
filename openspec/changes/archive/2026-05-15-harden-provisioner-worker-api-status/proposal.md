## Why

The Provisioner Worker currently has a few edge cases where its HTTP adapter and job lifecycle can bypass the intended UI-safe JSON error contract. This matters more now because future user-defined Workflow Presets will make preset identifiers and display names untrusted input instead of catalog-only data.

## What Changes

- Ensure every worker HTTP request path uses the same JSON error contract, including unsupported methods and unsupported endpoints with malformed or oversized bodies.
- Prevent unexpected preparation exceptions from printing unsanitized tracebacks after the worker has already recorded a safe terminal error.
- Stop echoing Custom Node display names and unsafe identifiers in diagnostics while allowing model asset display names during download progress and validated IDs for correlation.
- Add strict validation for preset-provided identifiers that may be returned as structured context in the future.
- Add regression tests for unsupported route/method handling, unexpected exception sanitization, and user-preset-safe diagnostics.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Tighten HTTP error routing, unexpected failure handling, and status diagnostic safety for future user-defined presets.

## Impact

- Affected code: `workers/provisioner/src/api/handler.py`, `workers/provisioner/src/orchestration/preparation_job.py`, `workers/provisioner/src/orchestration/preparer.py`, `workers/provisioner/src/runtime/validation.py`, and likely `workers/provisioner/src/app/schemas.py`.
- Affected tests: `workers/provisioner/tests/test_api.py`, `workers/provisioner/tests/test_preparer.py`, and possibly `workers/provisioner/tests/test_errors.py`.
- Public API impact: no endpoint additions or removals; error payloads become more consistent for unsupported methods and routes.
- Dependency impact: no new runtime dependencies.
