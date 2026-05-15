## 1. HTTP Adapter Hardening

- [x] 1.1 Refactor `ProvisionerRequestHandler` so method/path routing is resolved before reading or decoding request bodies.
- [x] 1.2 Preserve existing `/start` and `/cancel` body-size, content-length, JSON, and payload validation behavior for supported endpoints.
- [x] 1.3 Add worker JSON error handling for unsupported HTTP methods so stdlib HTML errors are not returned.
- [x] 1.4 Add API regression tests for unsupported `POST` endpoints with malformed JSON and oversized bodies.
- [x] 1.5 Add API regression tests for unsupported HTTP methods with missing/invalid authorization and with valid authorization.

## 2. Failure Sanitization

- [x] 2.1 Update `JobManager._run` to record sanitized `unexpected_error` terminal failures without re-raising the original exception.
- [x] 2.2 Add a job lifecycle test proving unexpected exceptions do not expose the original exception message in status payloads.
- [x] 2.3 Add a regression test proving the worker thread does not emit default traceback output for unexpected preparation exceptions.

## 3. User-Preset-Safe Diagnostics

- [x] 3.1 Add schema validation for preset identifiers used by the worker, including Workflow Preset IDs, Custom Node IDs, and model asset IDs.
- [x] 3.2 Ensure identifier validation rejects unsafe characters, path separators, control characters, non-ASCII characters, and identifiers longer than 128 characters without echoing raw values.
- [x] 3.3 Replace Custom Node progress names with validated-ID diagnostics while keeping model asset download names.
- [x] 3.4 Replace validation failure messages that include preset-provided item display names or unsafe identifiers with validated-ID diagnostics.
- [x] 3.5 Add tests proving malicious preset names and unsafe identifiers are not reflected through progress, terminal status, or error payload messages.

## 4. Verification

- [x] 4.1 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 4.2 Run `PYTHONPATH=src python3 -m compileall src tests` from `workers/provisioner`.
- [x] 4.3 Review `workers/provisioner/pyproject.toml` to confirm no new HTTP framework dependency was added.
