## Why

LumaForge needs a concrete Provisioner Worker runtime before native Workspace Provisioning can safely create a temporary RunPod pod and observe remote environment preparation. The current catalog already references a provisioner image, but the worker API, job lifecycle, container boundary, and preset asset write paths are not yet specified.

## What Changes

- Add a Python-based Provisioner Worker under `/workers/provisioner` with a container image build boundary.
- Expose a worker HTTP API with `POST /start`, `POST /cancel`, and `GET /status`.
- Start environment preparation only when `/start` is called with a selected Workflow Preset payload.
- Reject a second `/start` while a provisioning job is active.
- Prepare the mounted workspace volume by installing ComfyUI, optional Custom Nodes, and public Hugging Face model assets declared by the selected preset.
- Support only public Hugging Face downloads in v1; no Hugging Face API key or secret handling is introduced.
- Require Workflow Preset model assets to declare explicit ComfyUI-relative write paths.
- Keep native Workspace Provisioning orchestration, RunPod pod creation, endpoint creation, and React UI changes outside this change.

## Capabilities

### New Capabilities

- `provisioner-worker`: Container-side HTTP worker runtime that prepares a mounted ComfyUI workspace from a selected Workflow Preset and reports provisioning progress.

### Modified Capabilities

- `workspace-setup`: Bundled Workflow Presets must include explicit model asset write paths so the worker does not infer ComfyUI placement from model kind.

## Impact

- Affected worker code: new `/workers/provisioner` Python service, worker API schemas, job runner, container image definition, and worker tests.
- Affected catalog data: bundled Workflow Catalog model asset entries gain explicit ComfyUI-relative write paths.
- Affected reference/domain contracts: Model Asset / Workflow Preset structures gain asset installation path data.
- Affected external systems: public Hugging Face file downloads and Git repositories declared by Workflow Presets and Custom Nodes.
- Not affected: Tauri native provisioning commands, RunPod resource mutation, secure keyring, generated frontend command bindings, and Endpoint Worker runtime.
