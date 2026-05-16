# prepared-runtime-environment Specification

## Purpose

Define the prepared runtime metadata and volume-local Python environment shared by the Provisioner Worker and Endpoint Worker.
## Requirements
### Requirement: Prepare a volume-local Python runtime
The prepared workspace SHALL include a volume-local Python runtime materialized from the resolved runtime contract implementation's Docker-build-produced ComfyUI runtime archive.

#### Scenario: Image-baked runtime archive is materialized
- **WHEN** the Provisioner Worker prepares a workspace
- **THEN** it SHALL extract the resolved runtime contract implementation's image-baked runtime archive into a staging path under the mounted workspace volume
- **AND** it SHALL publish the staged runtime under the mounted workspace volume only after archive extraction and runtime validation succeed
- **AND** the materialized virtual environment path SHALL be `/workspace/.venv`
- **AND** the materialized ComfyUI root path SHALL be `/workspace/ComfyUI`
- **AND** the materialized virtual environment SHALL have been built with the final `/workspace/.venv` prefix before it was packaged into the image

#### Scenario: Provisioning does not resolve runtime dependencies
- **WHEN** the Provisioner Worker prepares a workspace
- **THEN** it MUST NOT create a fresh virtual environment for deterministic ComfyUI runtime dependencies
- **AND** it MUST NOT run `pip install` for ComfyUI base requirements
- **AND** it MUST NOT clone ComfyUI as part of deterministic runtime preparation
- **AND** provisioning MAY install Workflow Preset Custom Node sources and requirements after the base runtime archive is materialized

#### Scenario: Selected GPU does not change Python dependencies
- **WHEN** a workspace is prepared for any selected GPU
- **THEN** the prepared base Python dependency set SHALL come from the resolved runtime contract
- **AND** Workflow Preset Custom Node dependencies SHALL come from the selected Workflow Preset rather than selected GPU placement
- **AND** the selected GPU MUST NOT add, remove, replace, or reinstall Python packages during preparation

### Requirement: Record prepared runtime metadata
The prepared workspace SHALL include runtime metadata that describes the resolved runtime contract, materialized image-baked base runtime, preset-installed Custom Nodes, and required workspace assets.

#### Scenario: Runtime manifest is written
- **WHEN** runtime archive materialization, Custom Node preparation, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL write a runtime manifest under the mounted workspace volume
- **AND** the manifest SHALL include the environment kind, runtime contract id and version, selected implementation revision, concrete image identity, Python interpreter path, ComfyUI root path, Python version, platform, ComfyUI revision, preset-installed Custom Node revisions, and materialized timestamp
- **AND** the manifest SHALL identify that the Python runtime was materialized from the worker image rather than resolved during provisioning

#### Scenario: Build-time dependency records are referenced
- **WHEN** the image-baked runtime archive includes dependency records from Docker build
- **THEN** the prepared runtime metadata SHALL reference or copy those records into UI-safe workspace metadata
- **AND** those records MUST NOT imply that dependencies were installed during provisioning
- **AND** those records MUST NOT include provider API keys, worker bearer tokens, Hugging Face API keys, or other secrets

#### Scenario: Manifest is written after successful preparation
- **WHEN** runtime archive materialization fails, asset download fails, validation fails, or provisioning is cancelled
- **THEN** the Provisioner Worker MUST NOT write a terminal success runtime manifest for that workspace

### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL validate that the mounted prepared runtime environment has the required materialized image-baked runtime shape before running generation.

#### Scenario: Runtime environment is valid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest declares the expected resolved runtime contract, selected implementation revision, and materialized image-baked runtime
- **AND** the volume-local Python interpreter, ComfyUI entrypoint, required workflow file, required model file, and required Custom Node paths exist
- **THEN** the Endpoint Worker SHALL start ComfyUI through the materialized volume-local Python interpreter

#### Scenario: Runtime manifest is invalid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest is missing, invalid, or does not declare the expected resolved runtime contract, selected implementation revision, and materialized image-baked runtime
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared runtime by installing dependencies

#### Scenario: Volume environment is incomplete
- **WHEN** the Endpoint Worker starts generation
- **AND** the materialized Python interpreter, ComfyUI entrypoint, required workflow file, required model file, or required Custom Node path is missing
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe missing environment error
- **AND** it MUST NOT run pip, clone repositories, create virtual environments, or download model assets to repair the workspace

### Requirement: Run ComfyUI through the prepared volume environment
The Endpoint Worker SHALL execute ComfyUI with the Python interpreter from the materialized workspace runtime environment.

#### Scenario: ComfyUI is started by endpoint worker
- **WHEN** the Endpoint Worker needs to start ComfyUI for a valid generation request
- **THEN** it SHALL execute the ComfyUI entrypoint with the Python interpreter declared by the prepared runtime manifest
- **AND** it SHALL use the materialized ComfyUI root from the mounted workspace volume

#### Scenario: Endpoint container Python is not used for ComfyUI
- **WHEN** the Endpoint Worker executes ComfyUI
- **THEN** it MUST NOT execute ComfyUI through the endpoint container's default Python environment unless that interpreter is the same materialized workspace interpreter declared by the runtime manifest

### Requirement: Prepared runtime dependency records are workspace-resolved
The prepared runtime manifest SHALL advertise base dependency record paths that resolve under the mounted workspace and correspond to files published during runtime materialization.

#### Scenario: Manifest dependency records resolve under workspace
- **WHEN** the Provisioner Worker writes the prepared runtime manifest
- **THEN** every `base_dependency_record_paths` entry SHALL resolve under the mounted workspace path
- **AND** each entry SHALL identify a file that exists before terminal success is reported

#### Scenario: Relative catalog record paths are converted before manifest write
- **WHEN** the resolved runtime metadata contains relative base dependency record paths from the Runtime Catalog
- **THEN** the Provisioner Worker SHALL convert them to workspace-resolved manifest paths
- **AND** the Endpoint Worker MUST NOT resolve those manifest paths relative to its process working directory

### Requirement: Runtime archive format is extractable by provisioner
The runtime archive format declared by the Runtime Catalog implementation metadata SHALL be extractable by the Provisioner Worker runtime shipped in the corresponding provisioner image.

#### Scenario: Runtime archive is materialized
- **WHEN** the Provisioner Worker receives a start request for its matching resolved runtime implementation
- **THEN** it SHALL extract the configured runtime archive using a supported decompression path
- **AND** it SHALL publish the staged ComfyUI tree, virtual environment, and base runtime dependency records before writing the terminal runtime manifest

#### Scenario: Unsupported archive compression fails safely
- **WHEN** the configured runtime archive exists but cannot be decompressed or opened by the Provisioner Worker
- **THEN** the Provisioner Worker SHALL fail the active preparation job
- **AND** it MUST NOT write a terminal success runtime manifest
