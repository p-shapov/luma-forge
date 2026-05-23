# AGENTS.md

## Worker Scope

- This directory contains Python workers and worker contract promotion tooling.
- `provisioner/` is the container-side workspace preparation worker.
- `runpod-endpoint/` is the RunPod Serverless runtime worker for prepared ComfyUI environments.
- Keep worker runtime contracts explicit and compatible with bundled catalogs and the native provisioning flow.

---

## Structure

- `provisioner/src/`: Provisioner Worker implementation.
- `provisioner/tests/`: Provisioner Worker tests.
- `runpod-endpoint/src/`: RunPod Endpoint Worker implementation.
- `runpod-endpoint/tests/`: RunPod Endpoint Worker tests.
- `promote-provisioner-contract/`: Provisioner Catalog promotion tooling and tests.
- `promote-runtime-contract/`: Runtime contract schema, metadata, and Runtime Catalog promotion tooling.

---

## Contract Rules

- Keep request/response shapes compatible with the corresponding catalog metadata and native callers.
- Keep container runtime assumptions documented in the worker README or contract metadata when they change.

---

## Verification

For provisioner worker changes, run from the repository root:

- `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`

For RunPod endpoint worker changes, run from the repository root:

- `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests`
