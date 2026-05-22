# prepared-runtime-environment Specification

## Purpose

Define the prepared runtime metadata and workspace paths shared by the Provisioner Worker and Endpoint Worker.
## Requirements
### Requirement: Record prepared runtime metadata
The prepared workspace SHALL include minimal metadata that describes workspace preparation, required workspace assets, and preparation time needed by the Endpoint Worker.

#### Scenario: Runtime manifest is written
- **WHEN** model assets and final validation complete successfully
- **THEN** the Provisioner Worker SHALL write a runtime manifest under the mounted workspace volume
- **AND** the manifest SHALL include manifest kind or version, workspace root path, required model asset paths, and prepared timestamp
- **AND** the manifest MUST NOT include runtime contract id, runtime contract version, implementation revision, provisioner image identity, endpoint image identity, Python version, platform, ComfyUI revision, image base dependency record paths, runtime manifest compatibility metadata, protected dependency policy version, endpoint image Python path, endpoint image ComfyUI root, or endpoint image runtime root

#### Scenario: Manifest is written after successful preparation
- **WHEN** asset download fails, validation fails, or provisioning is cancelled
- **THEN** the Provisioner Worker MUST NOT write a terminal success runtime manifest for that workspace

### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL validate that the mounted prepared workspace contains the files needed to run generation with the fixed image-baked runtime environment.

#### Scenario: Runtime environment is valid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest declares required workspace-specific prepared paths
- **AND** the Endpoint Worker image-local fixed Python interpreter and ComfyUI entrypoint exist
- **AND** the required workflow file and required model file exist in the mounted workspace
- **THEN** the Endpoint Worker SHALL start ComfyUI through the fixed image-baked Python interpreter with the declared workspace paths

#### Scenario: Runtime manifest is invalid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest is missing, invalid, or does not declare required workspace-specific prepared paths
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared runtime by installing dependencies

#### Scenario: Workspace environment is incomplete
- **WHEN** the Endpoint Worker starts generation
- **AND** the fixed image runtime, required workflow file, or required model file is missing
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe missing environment error
- **AND** it MUST NOT run pip, clone repositories, create virtual environments, copy base runtime files, or download model assets to repair the workspace

### Requirement: Use image-baked base runtime with workspace assets
The prepared workspace SHALL reference a fixed image-baked base runtime and SHALL contain only workspace-specific assets, workflows, outputs, and metadata.

#### Scenario: Workspace-specific paths are prepared
- **WHEN** the Provisioner Worker prepares a workspace successfully
- **THEN** the mounted workspace SHALL contain workspace-specific model asset paths, workflow paths, output paths, and `.luma-forge` metadata paths
- **AND** the mounted workspace MUST NOT require a workspace-local virtual environment, ComfyUI checkout, runtime extension checkout directory, or dependency overlay to represent the deterministic runtime
- **AND** the mounted workspace MUST NOT contain provisioner-written endpoint Python, ComfyUI root, or image runtime root metadata

### Requirement: Run ComfyUI through the image runtime and workspace paths
The Endpoint Worker SHALL execute ComfyUI with the fixed image-baked Python interpreter and fixed image-baked ComfyUI root while using workspace-specific model, workflow, and output paths.

#### Scenario: ComfyUI is started by endpoint worker
- **WHEN** the Endpoint Worker needs to start ComfyUI for a valid generation request
- **THEN** it SHALL execute the fixed image-baked ComfyUI entrypoint with the fixed image-baked Python interpreter
- **AND** it SHALL configure ComfyUI to use workspace model and output paths
- **AND** it MUST NOT install dependencies, run pip, or mutate the image-baked Python environment
