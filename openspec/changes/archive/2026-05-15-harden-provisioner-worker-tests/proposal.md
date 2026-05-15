## Why

The provisioner worker test suite already covers the main happy paths and several hardened API behaviors, but recent audit findings identified gaps around invalid-start side effects, terminal error mapping, symlink path escapes, real cancellation behavior, and production container validation. Closing these gaps now reduces regression risk before the native provisioning flow depends on the worker as a remote execution boundary.

## What Changes

- Add regression tests proving invalid `POST /start` requests leave the worker idle, do not call preparation, and do not write to the workspace.
- Add a terminal job error mapping matrix for expected provisioner failure classes, including dependency install, asset download, asset authorization, path validation, timeout, and unexpected exceptions.
- Add path-safety regression tests for symlink escapes from workspace, ComfyUI, Custom Node, metadata, virtual environment, and model asset paths.
- Add real `Provisioner.prepare()` cancellation and partial-output tests around phase boundaries and asset placement.
- Add deployment validation so provisioner worker image changes run the existing container smoke test before publishing.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `provisioner-worker`: Require test coverage for invalid-start no-side-effect guarantees, terminal error mapping, symlink path safety, and cancellation/partial-output behavior.
- `worker-deployment`: Require provisioner deployment validation to include the container smoke check for provisioner image changes.

## Impact

- Affected tests: `workers/provisioner/tests/test_api.py`, `workers/provisioner/tests/test_preparer.py`, `workers/provisioner/tests/test_paths.py`, and possibly new focused provisioner test modules.
- Affected CI/deployment automation: GitHub Actions worker deployment workflow and provisioner deployment documentation if container smoke execution changes.
- No worker API, request/response contract, runtime dependencies, or user-facing behavior changes are intended.
