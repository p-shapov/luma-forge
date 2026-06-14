# AGENTS.md

## Worker Scope

- This directory contains Python workers and worker contract promotion tooling.
- `provisioner/` is the container-side workspace preparation worker.
- `runpod-endpoint/` is the RunPod Serverless runtime worker for prepared ComfyUI environments.
- Keep worker runtime contracts explicit and compatible with bundled catalogs and the native provisioning flow.

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
