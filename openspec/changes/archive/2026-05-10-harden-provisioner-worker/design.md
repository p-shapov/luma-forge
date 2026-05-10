## Context

The Provisioner Worker runs inside the remote provisioning container and exposes a small HTTP API used by the native provisioning flow. The current implementation already avoids provider secrets in API responses and validates several filesystem paths, but it still assumes a trusted caller, accepts request bodies without an explicit size boundary, collapses several operational failures into generic preparation errors, and prepares Git sources from revisions that may move over time.

Model assets are public Hugging Face files. The catalog contains only the metadata needed to locate and place each file: repository id, file path, revision, and install path. This change keeps that model and relies on Hugging Face Hub for download resolution and caching behavior.

## Goals / Non-Goals

**Goals:**

- Add an optional bearer-token gate to every worker endpoint without introducing provider or Hugging Face secrets into worker responses.
- Bound request body size and reject malformed request metadata before parsing JSON.
- Use specific, UI-safe error codes for authorization, validation, Git checkout, dependency installation, Hugging Face download/auth failures, timeouts, cancellation, and conflicts.
- Require immutable commit revisions for bundled ComfyUI and Custom Node Git sources.
- Download public Hugging Face model assets through Hub semantics using repository id, file path, revision, and install path.
- Add timeouts for external work so failed Git, pip, or download operations cannot hang indefinitely.

**Non-Goals:**

- Add private or gated Hugging Face model support.
- Add Hugging Face token storage or forwarding.
- Add model digest metadata or app-owned checksum verification.
- Change the native provider provisioning lifecycle or RunPod API contract.
- Redesign the worker protocol beyond the hardening and error taxonomy required here.

## Decisions

1. **Use optional bearer-token authorization for the worker API.**

   The worker SHALL require `Authorization: Bearer <token>` when `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` is configured. When the token is absent, local development and tests can continue using the API without auth. This keeps the worker deployable in both dev and remote provisioning environments while making production hardening a configuration choice controlled by the native provisioning path.

   Alternative considered: always require auth. That is stricter, but it would make local worker development and smoke tests more cumbersome before the native RunPod path is wiring token injection end to end.

2. **Rely on Hugging Face Hub caching for model assets.**

   The model asset contract SHALL remain repository id, file path, revision, and install path. The worker SHALL use Hugging Face Hub download behavior to resolve and cache public files. The app catalog SHALL NOT carry unused model asset metadata.

   Alternative considered: adding `sha256_digest` to every model asset and verifying it after download. That gives stronger app-owned integrity checking, but for v1 it adds catalog maintenance and long-running verification work without a concrete requirement for private mirroring or independent artifact attestation.

3. **Pin bundled Git sources to immutable commits.**

   The bundled Workflow Catalog SHALL use full 40-character Git commit revisions for ComfyUI and Custom Nodes. Native catalog validation SHALL reject mutable branch or tag names for those Git sources. This makes worker-prepared source code reproducible and avoids silent drift in the runtime code.

   Alternative considered: keep mutable branch names and rely on worker fetch behavior. That keeps the catalog simpler, but it makes the same app build prepare different source code over time.

4. **Return stable error codes with UI-safe messages.**

   Worker exceptions SHALL map to explicit codes such as `unauthorized`, `request_too_large`, `git_checkout_failed`, `dependency_install_failed`, `asset_download_failed`, `asset_auth_required`, `path_validation_failed`, `step_timeout`, and `cancelled`. Messages SHALL remain safe for UI display and MUST NOT include secrets or raw credential-bearing command output.

   Alternative considered: continue returning only generic `invalid_request` and `preparation_failed` responses. That is simpler, but it makes native retry decisions and user-facing diagnostics ambiguous.

5. **Bound external work with configurable timeouts.**

   Git commands, dependency installation, and model downloads SHALL use worker configuration defaults that can be overridden by environment variables. Git and dependency subprocesses SHALL be terminated on timeout. Hugging Face downloads SHALL run in an isolated child process so timeout or cancellation can terminate the active download before the job reports `step_timeout`.

   Alternative considered: rely on provider-level pod timeouts. That still leaves users with long periods of unclear progress and weaker cancellation behavior.

## Risks / Trade-offs

- Hugging Face Hub download behavior does not expose chunk-level cancellation controls. Mitigation: run each Hub download in an isolated child process and terminate that process on timeout or cancellation before reporting terminal failure.
- Optional API authorization can be misconfigured off in production. Mitigation: native provisioning should set a token when launching remote workers, and tests should cover both configured and unconfigured modes.
- Immutable Git revisions require catalog maintenance when upstream ComfyUI or Custom Nodes update. Mitigation: update the bundled catalog intentionally as part of app releases.
- Keeping the model asset contract minimal means future UI/storage estimates will need a separate metadata lookup. Mitigation: use Hugging Face metadata APIs later if the UI needs those estimates.

## Migration Plan

1. Update the bundled Workflow Catalog to replace mutable Git revisions with immutable commits.
2. Keep model asset records digest-free and size-free; update Hugging Face repository id, file path, revision, and install path when the catalog intentionally moves to a new public asset revision.
3. Add worker configuration, auth, request size checks, timeout handling, and structured error mapping.
4. Install the worker package in the production Docker image so Python dependencies declared by `pyproject.toml` are available at runtime.
5. Update native catalog validation and generated command bindings only where existing type generation requires it.
6. Add or update worker, native, and opt-in container smoke tests for auth, request limits, immutable Git revision validation, model download error codes, timeout behavior, and production dependency installation.
7. Roll back by clearing the worker auth token configuration and reverting the catalog/source validation change if the native provisioning path cannot yet provide immutable revisions.

## Open Questions

None.
