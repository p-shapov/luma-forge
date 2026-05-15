## Why

The provisioner worker source tree is becoming easier to maintain at the module level, but it is still organized around a single `provisioner_worker` package whose root is too flat. Since `workers/provisioner` contains one worker application, grouping responsibility-based packages directly under `src/` will make the source tree easier to scan and avoid an unnecessary package wrapper.

## What Changes

- Reorganize `workers/provisioner/src/` into top-level responsibility-based packages instead of keeping all worker code under `src/provisioner_worker/`.
- Keep the worker entry point obvious and update launch wiring away from `python -m provisioner_worker`.
- Keep app-wide config, schemas, and error contracts in the top-level app package.
- Move HTTP request handling into a top-level API package.
- Move job lifecycle management and the high-level runtime-preparation use case into a top-level orchestration package.
- Move prepared runtime environment paths, manifest, Python environment, dependency records, and validation into a top-level runtime package.
- Move Git checkout, Hugging Face retrieval, path containment helpers, and generic process execution into a single top-level auxiliary package.
- Update packaging, Docker launch commands, README commands, and tests to use the new top-level source layout.
- Preserve all current runtime behavior, external HTTP API behavior, error payloads, progress phases, prepared filesystem outputs, and tests.
- Avoid new runtime dependencies.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Clarifies internal package organization while preserving existing worker behavior and contracts.

## Impact

- Affected code: module paths under `workers/provisioner/src/` and imports in the provisioner worker source and tests.
- Affected tests: provisioner worker tests that import moved modules directly.
- Affected docs and launch wiring: `workers/provisioner/README.md`, `workers/Dockerfile`, and `workers/provisioner/pyproject.toml` if package discovery or entrypoint metadata must change.
- APIs: no external HTTP API, request payload, response payload, status, progress, or manifest contract changes.
- Dependencies: no new runtime dependencies.
