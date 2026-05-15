## Why

The Provisioner Worker currently accepts unauthenticated requests when no bearer token is configured, while the container image binds the worker API to all interfaces. This makes deployment safety depend on external network isolation and leaves invalid runtime configuration easy to miss.

## What Changes

- **BREAKING**: Require `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` for Provisioner Worker startup.
- **BREAKING**: Reject blank, malformed, non-positive, or out-of-range Provisioner Worker runtime environment values during startup instead of silently falling back to defaults.
- Keep local development explicit by requiring developers and tests to provide a valid token when exercising the HTTP API.
- Document and test the accepted runtime environment contract for bind host, port, request size, timeouts, workspace mount path, and bearer token.
- Ensure unauthorized worker API requests cannot read status or mutate provisioning jobs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `provisioner-worker`: Require authenticated worker startup and strict validation for Provisioner Worker runtime environment variables.

## Impact

- Affected code: `workers/provisioner/src/provisioner_worker/config.py`, `server.py`, `api.py`, `job_manager.py`, worker tests, and worker documentation.
- Affected deployment: Provisioner containers must receive `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` and valid runtime env values before startup.
- Affected native provisioning path: when remote provisioning orchestration is implemented, the Native Layer must generate a per-pod bearer token, inject it into the Provisioner Worker environment, and send it on all worker API calls.
- Affected compatibility: existing unauthenticated local `docker run` and direct worker invocations must be updated to set a token.
