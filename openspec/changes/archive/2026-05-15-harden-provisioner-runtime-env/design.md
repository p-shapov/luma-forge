## Context

The Provisioner Worker is a temporary HTTP worker running inside a provider-side provisioning pod. It currently reads runtime environment values in multiple modules, permits unauthenticated API access when `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` is missing, silently falls back to defaults for malformed numeric values, and binds to `0.0.0.0` in the container image.

That combination makes production safety depend on network isolation outside the worker. Because `/start` can clone repositories and install dependencies into the mounted workspace, the worker must fail closed when its security-critical runtime configuration is invalid.

## Goals / Non-Goals

**Goals:**

- Require a valid bearer token before the Provisioner Worker starts serving HTTP.
- Validate all Provisioner Worker runtime environment values during startup.
- Replace silent fallback behavior with explicit startup failure for invalid configured values.
- Keep configuration parsing centralized so server binding, API auth, request limits, timeouts, and workspace mount validation use one authoritative config snapshot.
- Update tests and documentation so local and container runs pass a token explicitly.

**Non-Goals:**

- Do not implement the full native workspace provisioning orchestrator in this change.
- Do not add a hosted backend service or external secret manager.
- Do not support Hugging Face private model tokens.
- Do not change Provider API Key storage or provider setup behavior.

## Decisions

### Centralize Provisioner Worker runtime config parsing

Create one strict runtime configuration parser owned by `provisioner_worker.config`. The parser should return a typed config object containing host, port, bearer token, max request bytes, timeouts, and workspace mount path. `server.py`, `api.py`, `job_manager.py`, and `preparer.py` should receive or read this parsed config instead of reading environment values independently.

Alternative considered: keep the current module-local environment reads and add validation at each call site. That would preserve duplicated parsing rules and make future config changes harder to audit.

### Fail startup on invalid runtime configuration

Runtime environment values that affect security or resource bounds must be validated before the HTTP server starts. Missing bearer token, blank values, malformed numbers, non-positive limits, invalid ports, invalid host values, and invalid workspace mount paths should raise a clear configuration error and exit before binding a socket.

Alternative considered: continue falling back to safe defaults for malformed values. This is too easy to miss in deployment and can silently disable intended controls.

### Require bearer auth in all modes

Remove unauthenticated worker mode. A valid `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` is required for local runs, tests, container smoke tests, and remote provisioning. The token should be bearer-header-safe: trimmed value must be non-empty, have sufficient length for generated secrets, and contain no whitespace or control characters.

Alternative considered: require the token only when binding to a non-loopback address. That preserves a development exception but keeps two security modes and makes container behavior depend on bind-host configuration.

### Keep the token out of durable state and logs

The worker should compare the `Authorization` header with the configured token but must not return, persist, or log the token. Unauthorized responses should remain UI-safe and generic.

### Prepare for native per-pod tokens

This change should document that the future native provisioning path must generate a per-pod bearer token, inject it into the Provisioner Worker environment, and use it for all `/status`, `/start`, and `/cancel` calls. The implementation of that native provisioning path remains outside this change unless the relevant orchestration code already exists when implementation begins.

## Risks / Trade-offs

- Existing local commands fail until a token is set -> update README examples, tests, and smoke commands with explicit token usage.
- Test fixtures become noisier because auth is mandatory -> add helper defaults that set a valid test token and make unauthenticated cases explicit startup/config tests instead of API success tests.
- Operators may pass weak static tokens -> enforce minimum token shape in the worker and document that the Native Layer should generate high-entropy per-pod tokens.
- Strict host validation can reject provider-specific bind names -> accept valid IP addresses and non-empty DNS hostnames, while keeping socket bind errors as startup failures.
