## Why

Provisioner startup configuration errors now have a machine-readable shape, but running worker API errors and job status errors still use only `code` and `message`. Aligning these payloads gives the native provisioning consumer one predictable error contract for startup diagnostics, request failures, and terminal job failures.

## What Changes

- Standardize Provisioner Worker HTTP error responses around `code`, `reason_code`, `message`, and optional structured context.
- Standardize terminal job failure metadata in `GET /status` to use the same structured worker error shape.
- Keep all error payloads UI-safe and secret-free.
- Preserve existing stable `code` values as the primary classification for compatibility.
- Add reason-level classification for validation, authorization, conflict, request-size, not-found, and preparation failures.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `provisioner-worker`: Standardize API and job failure error payloads to match the structured configuration error format.

## Impact

- Affected code: `workers/provisioner/src/provisioner_worker/errors.py`, `api.py`, `job_manager.py`, worker tests, and provisioner README.
- Affected API contract: error responses and failed job status metadata gain `reason_code`; conflict responses retain `active_job_id` as structured context.
- Affected consumer behavior: native provisioning should read `code` for broad classification and `reason_code` for specific handling.
