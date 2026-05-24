# Prepared Runtime Environment Specification

## Purpose
Define the minimal prepared workspace and endpoint runtime expectations for image-baked ComfyUI execution.

## Requirements

### Requirement: Execute image-baked runtime without provisioning-time setup
The Endpoint Worker SHALL execute the image-baked ComfyUI runtime and the selected baked workflow at request time without requiring provisioning to install, repair, or describe endpoint runtime files.

#### Scenario: Endpoint request executes prepared image runtime
- **WHEN** the Endpoint Worker handles a valid generation request
- **THEN** it SHALL use the endpoint image's ComfyUI checkout, Python environment, Comfy CLI installation, and baked workflow file
- **AND** it SHALL use the mounted workspace for provisioned model assets
- **AND** it MUST NOT require a provisioner-written runtime manifest

#### Scenario: Provisioner remains model-asset focused
- **WHEN** Workspace Provisioning prepares a workspace for the selected endpoint runtime
- **THEN** provisioning SHALL remain limited to workspace directories, model asset download or verification, and declared model asset validation
- **AND** provisioning MUST NOT clone ComfyUI, install endpoint Python dependencies, install Comfy CLI, run pip for endpoint dependencies, patch workflows, start ComfyUI, or validate generated outputs

### Requirement: Use image-baked base runtime with workspace assets
The prepared workspace SHALL contain workspace-specific assets and workflow files only when those files are needed by the selected endpoint runtime; it SHALL NOT require provisioner-written runtime metadata.

#### Scenario: Workspace-specific paths are prepared
- **WHEN** the Provisioner Worker prepares a workspace successfully
- **THEN** the mounted workspace SHALL contain required model asset files and any workspace directories needed for provisioning
- **AND** the mounted workspace MUST NOT require a workspace-local virtual environment, ComfyUI checkout, runtime extension checkout directory, dependency overlay, or `.luma-forge/runtime-manifest.json` to represent the deterministic runtime
- **AND** the mounted workspace MUST NOT contain provisioner-written endpoint Python, ComfyUI root, image runtime root, model asset path list, or prepared timestamp metadata as an endpoint contract
