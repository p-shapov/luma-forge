# prepared-runtime-environment Specification

## Purpose

Define the prepared runtime metadata and volume-local Python environment shared by the Provisioner Worker and Endpoint Worker.
## Requirements
### Requirement: Record prepared runtime metadata
The prepared workspace SHALL include runtime metadata that describes the resolved runtime contract, image-baked base runtime, preset-installed Custom Nodes, workspace overlay dependencies, and required workspace assets.

#### Scenario: Runtime manifest is written
- **WHEN** image runtime validation, Custom Node preparation, overlay dependency installation, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL write a runtime manifest under the mounted workspace volume
- **AND** the manifest SHALL include the environment kind, runtime contract id and version, selected implementation revision, concrete provisioner image identity, concrete endpoint image identity, image Python interpreter path, image ComfyUI root path, workspace overlay path, Python version, platform, ComfyUI revision, preset-installed Custom Node revisions, required model asset paths, protected dependency policy version, and prepared timestamp
- **AND** the manifest SHALL identify that the Python base runtime is image-baked and not materialized into the mounted workspace during provisioning

#### Scenario: Build-time dependency records are referenced
- **WHEN** the image-baked runtime includes dependency records from Docker build
- **THEN** the prepared runtime metadata SHALL reference those records as image-runtime metadata and SHALL copy or write only UI-safe workspace records needed for endpoint validation
- **AND** those records MUST NOT imply that base dependencies were installed during provisioning
- **AND** those records MUST NOT include provider API keys, worker bearer tokens, Hugging Face API keys, or other secrets

#### Scenario: Manifest is written after successful preparation
- **WHEN** image runtime validation fails, overlay dependency installation fails, asset download fails, validation fails, or provisioning is cancelled
- **THEN** the Provisioner Worker MUST NOT write a terminal success runtime manifest for that workspace

### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL validate that the mounted prepared workspace is compatible with the image-baked runtime environment before running generation.

#### Scenario: Runtime environment is valid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest declares the expected resolved runtime contract, selected implementation revision, image-baked runtime identity, workspace overlay policy, and endpoint image identity
- **AND** the image Python interpreter, image ComfyUI entrypoint, required workflow file, required model file, required Custom Node paths, and declared overlay paths exist
- **THEN** the Endpoint Worker SHALL start ComfyUI through the image-baked Python interpreter with the declared workspace overlay and workspace paths

#### Scenario: Runtime manifest is invalid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest is missing, invalid, or does not declare the expected resolved runtime contract, selected implementation revision, image-baked runtime identity, overlay policy, or endpoint image identity
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared runtime by installing dependencies

#### Scenario: Workspace environment is incomplete
- **WHEN** the Endpoint Worker starts generation
- **AND** the image runtime, required workflow file, required model file, required Custom Node path, or declared overlay path is missing
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe missing environment error
- **AND** it MUST NOT run pip, clone repositories, create virtual environments, copy base runtime files, or download model assets to repair the workspace

### Requirement: Prepared runtime dependency records are workspace-resolved
The prepared runtime manifest SHALL advertise workspace dependency and install record paths that resolve under the mounted workspace, and SHALL separately identify image-baked base dependency records as image runtime metadata.

#### Scenario: Manifest dependency records resolve under workspace
- **WHEN** the Provisioner Worker writes the prepared runtime manifest
- **THEN** every workspace dependency record, overlay install report, Custom Node record, and model asset record path SHALL resolve under the mounted workspace path
- **AND** each entry SHALL identify a file that exists before terminal success is reported

#### Scenario: Relative catalog record paths are converted before manifest write
- **WHEN** the resolved runtime metadata contains relative workspace record paths from the Runtime Catalog
- **THEN** the Provisioner Worker SHALL convert them to workspace-resolved manifest paths
- **AND** the Endpoint Worker MUST NOT resolve those manifest paths relative to its process working directory

#### Scenario: Image base dependency records are validated separately
- **WHEN** the resolved runtime metadata contains image base dependency record paths
- **THEN** the Provisioner Worker and Endpoint Worker SHALL validate those paths relative to the configured image runtime root
- **AND** they MUST NOT require those image base dependency records to be copied into the mounted workspace

### Requirement: Use image-baked base runtime with workspace overlay
The prepared workspace SHALL reference an immutable image-baked base runtime and SHALL contain only workspace-specific runtime data, dependency overlays, assets, outputs, and metadata.

#### Scenario: Provisioner validates image runtime
- **WHEN** the Provisioner Worker prepares a workspace
- **THEN** it SHALL validate that the running worker image exposes the resolved runtime contract implementation under the configured image runtime root
- **AND** it SHALL validate the image Python interpreter, image ComfyUI root, runtime identity metadata, and base dependency records before writing terminal workspace metadata
- **AND** it MUST NOT extract, copy, or publish the base virtual environment or base ComfyUI tree into the mounted workspace volume

#### Scenario: Workspace-specific paths are prepared
- **WHEN** the Provisioner Worker prepares a workspace successfully
- **THEN** the mounted workspace SHALL contain workspace-specific model asset paths, Custom Node checkout paths, output paths, `.luma-forge` metadata paths, and `.luma-forge/python-overlay`
- **AND** the mounted workspace MUST NOT require `/workspace/.venv` or `/workspace/ComfyUI` to represent the deterministic base runtime

#### Scenario: Custom Node overlay is recorded
- **WHEN** Custom Node dependency installation succeeds
- **THEN** the prepared workspace SHALL record the overlay site-packages path, install report paths, dependency records, and protected dependency policy version in the runtime manifest
- **AND** those records SHALL be sufficient for the Endpoint Worker to reproduce the import path configuration without running pip

### Requirement: Run ComfyUI through the image runtime and workspace overlay
The Endpoint Worker SHALL execute ComfyUI with the image-baked Python interpreter and image-baked ComfyUI root while adding workspace-specific Custom Node, model, output, and Python overlay paths.

#### Scenario: ComfyUI is started by endpoint worker
- **WHEN** the Endpoint Worker needs to start ComfyUI for a valid generation request
- **THEN** it SHALL execute the image-baked ComfyUI entrypoint with the image-baked Python interpreter declared by the prepared runtime manifest
- **AND** it SHALL configure ComfyUI to use workspace model paths, workspace Custom Node paths, workspace output paths, and the workspace Python overlay declared by the prepared runtime manifest

#### Scenario: Workspace overlay augments image runtime
- **WHEN** the Endpoint Worker starts ComfyUI with a prepared workspace overlay
- **THEN** it SHALL add the declared overlay site-packages path to the ComfyUI process import path according to the recorded overlay policy
- **AND** it MUST NOT install dependencies, run pip, or mutate the image-baked Python environment

