## 1. Error Payload Model

- [x] 1.1 Extend `WorkerError` with a canonical serialization method that returns `code`, `reason_code`, `message`, and optional `context`.
- [x] 1.2 Add default `reason_code` values for all existing worker error classes.
- [x] 1.3 Add safe context support for allowlisted metadata without exposing raw request bodies, tokens, command output, stack traces, or environment values.

## 2. API and Job Status Integration

- [x] 2.1 Update immediate HTTP error responses in `api.py` to use canonical worker error serialization.
- [x] 2.2 Update malformed JSON and content-length failures to carry specific stable `reason_code` values.
- [x] 2.3 Update conflict responses so active job metadata is exposed as structured safe context.
- [x] 2.4 Update terminal job failure metadata in `job_manager.py` to use the same canonical worker error shape.

## 3. Tests

- [x] 3.1 Add unit tests for worker error serialization, reason codes, and optional context omission.
- [x] 3.2 Update API tests to assert `reason_code` on unauthorized, invalid request, request-too-large, not-found, malformed JSON, and conflict responses.
- [x] 3.3 Update job status tests to assert failed job `error` includes `code`, `reason_code`, and `message`.
- [x] 3.4 Add regression tests proving unsafe values are not included in serialized worker errors.

## 4. Documentation and Verification

- [x] 4.1 Update provisioner README error response examples to include `reason_code` and structured `context`.
- [x] 4.2 Document that consumers should use `code` for broad classification and `reason_code` for specific handling.
- [x] 4.3 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 4.4 Run `PYTHONPATH=src python3 -m compileall src tests` from `workers/provisioner`.
