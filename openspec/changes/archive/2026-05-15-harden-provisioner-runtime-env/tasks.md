## 1. Runtime Configuration

- [x] 1.1 Add a strict Provisioner Worker runtime config parser that reads host, port, bearer token, request size limit, step timeouts, and workspace mount path in one place.
- [x] 1.2 Add configuration error types/messages that are UI-safe and never include bearer token values.
- [x] 1.3 Validate bearer token shape: required, trimmed, at least 32 characters, and free of whitespace or control characters.
- [x] 1.4 Validate numeric runtime env values: integer port in range, positive request byte limit, and positive finite timeout values.
- [x] 1.5 Validate bind host and workspace mount path before the HTTP server binds a socket.

## 2. Worker Integration

- [x] 2.1 Wire `server.py` to parse runtime config before constructing `ThreadingHTTPServer`.
- [x] 2.2 Pass the validated config into the request handler, job manager, and provisioner instead of re-reading environment variables in separate modules.
- [x] 2.3 Remove unauthenticated API mode so every `GET /status`, `POST /start`, and `POST /cancel` request requires the configured bearer token.
- [x] 2.4 Ensure unauthorized requests cannot expose status, start jobs, cancel jobs, or include the configured token in responses.
- [x] 2.5 Emit machine-readable startup configuration failures with error code `configuration_error`.

## 3. Tests

- [x] 3.1 Add unit tests for valid runtime config parsing and every invalid env class in the spec.
- [x] 3.2 Update API tests so the default fixture sends a valid bearer token and missing or wrong authorization returns `401`.
- [x] 3.3 Add startup-level tests proving invalid runtime envs fail before the HTTP server starts.
- [x] 3.4 Update container smoke tests to run the provisioner with an explicit bearer token and call `/status` with authorization.
- [x] 3.5 Add tests for structured startup configuration error payloads.

## 4. Documentation and Verification

- [x] 4.1 Update provisioner README local, container, and API examples to include bearer token setup and authorization headers.
- [x] 4.2 Document the runtime env validation contract and note that native provisioning must inject a per-pod token.
- [x] 4.3 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 4.4 Run `PYTHONPATH=src python3 -m compileall src tests` from `workers/provisioner`.
- [x] 4.5 Document startup configuration error payloads and exit behavior.
