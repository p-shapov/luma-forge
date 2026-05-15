## Why

`workers/provisioner/src/provisioner_worker/preparer.py` has grown into a broad module that mixes preparation orchestration with subprocess management, Git operations, Python environment management, Hugging Face downloads, filesystem placement, dependency recording, and validation. This makes the critical runtime preparation flow harder to review and raises the cost of adding future provisioning behavior safely.

## What Changes

- Refactor the provisioner worker preparation implementation so `preparer.py` focuses on high-level workflow orchestration.
- Move subprocess execution and cancellation handling into a dedicated command execution module.
- Move public model asset download behavior, including Hugging Face isolation and timeout handling, into a dedicated download module.
- Move Git checkout behavior, Python environment setup, dependency installation/recording, and prepared environment validation into focused helper modules or services.
- Preserve the current provisioner worker API, job lifecycle behavior, progress phases, error mapping, runtime manifest behavior, and filesystem outputs.
- Preserve the existing test coverage while relocating or adding tests around the new module boundaries.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Clarifies internal preparation boundaries while preserving existing provisioning behavior and worker contracts.

## Impact

- Affected code: `workers/provisioner/src/provisioner_worker/preparer.py` and new focused modules under `workers/provisioner/src/provisioner_worker/`.
- Affected tests: `workers/provisioner/tests/test_preparer.py` and any new tests for extracted modules.
- APIs: no external HTTP API, request payload, response payload, status, progress, or manifest contract changes.
- Dependencies: no new runtime dependencies.
