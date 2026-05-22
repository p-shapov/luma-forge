# Prepared Runtime Environment Specification

## Purpose
Define the minimal prepared workspace and endpoint runtime expectations while generation is stubbed.

## Requirements
### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL keep runtime validation limited to the temporary stub boundary until real generation is reintroduced.

#### Scenario: Stub runtime environment is valid
- **WHEN** the Endpoint Worker handles a generation request
- **THEN** the Endpoint Worker SHALL return the configured stub response through the RunPod handler boundary
- **AND** it MUST NOT require or read a prepared runtime manifest
- **AND** it MUST NOT prevalidate configured workflow files, required model files, output directories, fixed Python interpreters, or fixed ComfyUI entrypoints

#### Scenario: Stub does not repair runtime environment
- **WHEN** the Endpoint Worker handles startup or a generation request
- **THEN** it MUST NOT run pip, clone repositories, create virtual environments, copy base runtime files, download model assets, start ComfyUI, or read a manifest to repair the environment

### Requirement: Use image-baked base runtime with workspace assets
The prepared workspace SHALL contain workspace-specific assets and workflow files only when those files are needed by the selected endpoint runtime; it SHALL NOT require provisioner-written runtime metadata.

#### Scenario: Workspace-specific paths are prepared
- **WHEN** the Provisioner Worker prepares a workspace successfully
- **THEN** the mounted workspace SHALL contain required model asset files and any workspace directories needed for provisioning
- **AND** the mounted workspace MUST NOT require a workspace-local virtual environment, ComfyUI checkout, runtime extension checkout directory, dependency overlay, or `.luma-forge/runtime-manifest.json` to represent the deterministic runtime
- **AND** the mounted workspace MUST NOT contain provisioner-written endpoint Python, ComfyUI root, image runtime root, model asset path list, or prepared timestamp metadata as an endpoint contract

### Requirement: Defer ComfyUI execution to a future runtime implementation
The Endpoint Worker SHALL NOT execute ComfyUI while the endpoint runtime is stubbed.

#### Scenario: Stubbed endpoint receives generation request
- **WHEN** the Endpoint Worker receives a valid generation request
- **THEN** it SHALL return the stubbed response without executing ComfyUI
- **AND** it MUST NOT install dependencies, run pip, mutate the image-baked Python environment, validate workflow/model/output paths, or read a provisioner-written manifest
