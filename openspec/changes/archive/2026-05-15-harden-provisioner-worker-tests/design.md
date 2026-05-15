## Context

The Provisioner Worker is the container-side preparation boundary for ComfyUI workspaces. Its current Python test suite passes and already covers authorization, request parsing, configuration validation, immutable Git revisions, volume-local dependency installation, runtime manifest shape, and several failure paths.

The remaining audit gaps are concentrated in regression coverage rather than product behavior. The documented worker contract already requires invalid starts to avoid side effects, path validation to prevent writes outside mounted workspace paths, terminal failures to expose stable UI-safe error codes, cancellation to stop active preparation, and deployment workflows to validate worker images before publishing.

## Goals / Non-Goals

**Goals:**

- Add tests that directly fail when invalid `POST /start` requests call preparation, mutate job state, or write to the workspace.
- Add compact coverage for all expected terminal worker error classes mapped through job status.
- Add symlink escape tests that exercise path resolution against real filesystem objects.
- Add real `Provisioner.prepare()` cancellation and partial-output tests without depending on external Git, pip, or network calls.
- Run the existing provisioner container smoke test during provisioner worker deployment validation after the image is built and before publishing.

**Non-Goals:**

- Do not change the worker HTTP API, error payload shape, runtime manifest contract, or status lifecycle.
- Do not add new worker runtime dependencies.
- Do not perform real GitHub, Hugging Face, or pip network operations in normal unit tests.
- Do not make the endpoint worker deployment workflow run provisioner-specific smoke tests.

## Decisions

### Keep regression tests at the narrowest boundary that proves the contract

Invalid-start and terminal-error mapping tests belong at the API or `JobManager` boundary because those are the public status and error surfaces used by Native orchestration. Path escape tests belong in `auxiliary.paths` and runtime validation tests because those functions are the shared guardrails before filesystem writes. Cancellation tests belong around `Provisioner.prepare()` with fake collaborators that honor `cancel_event`, so the real phase sequencing is exercised without external infrastructure.

Alternative considered: run a full local HTTP server and real subprocess stack for every failure case. That would provide broad integration coverage, but it would make the suite slower and more fragile while duplicating lower-level subprocess and downloader tests.

### Use table-driven matrices for stable error mapping

Terminal error mapping should use a table of expected worker error classes and expected `code` / `reason_code` values. Each case should drive the same failing provisioner shape and assert terminal `failed` status, diagnostic message safety, and error payload classification.

Alternative considered: keep one test per error class. That is easier to read in isolation but increases boilerplate and makes it easier for a new worker error class to be added without updating the matrix.

### Test symlink escapes with real temporary directories

Symlink tests should create temporary workspace trees with symlinks that point outside the workspace, then assert path helpers or validation fail before writes. The tests should cover generic child paths and custom-node paths because both are used by asset placement, metadata paths, runtime paths, and Custom Node checkout paths.

Alternative considered: only test lexical path traversal such as `../`. Those tests already exist and do not catch existing filesystem objects that resolve outside the mount.

### Keep container smoke opt-in locally but mandatory in deployment validation

The existing `LUMA_FORGE_RUN_CONTAINER_SMOKE=1` gate should remain for local developer runs because the test requires Docker. The provisioner deployment workflow should invoke the smoke test after building the provisioner image and before GHCR login/publish, using the built image tag rather than rebuilding a different image when practical.

Alternative considered: always run container smoke as part of `python -m unittest discover`. That would make normal development and sandboxed runs depend on Docker availability and local port binding.

## Risks / Trade-offs

- Docker is not always available on CI runners or local machines -> Keep smoke opt-in locally and run it only in the deployment workflow where Docker is already required for image publication.
- Cancellation tests can become timing-sensitive -> Prefer deterministic fake collaborators that set events and wait on explicit release/cancel signals instead of sleeping.
- Symlink behavior differs slightly across platforms -> Scope the worker to macOS development and Linux containers, and write tests using standard `Path.symlink_to` with skips only if the platform disallows symlink creation.
- More API tests that bind loopback ports can be blocked by sandboxed environments -> Keep job-manager-level tests for matrix coverage where full HTTP behavior is not required.
