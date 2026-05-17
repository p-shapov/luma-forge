## MODIFIED Requirements

### Requirement: Record prepared runtime metadata
The prepared workspace SHALL include minimal runtime metadata that describes workspace-specific Custom Nodes, overlay dependencies, required workspace assets, and fixed runtime paths needed by the Endpoint Worker.

#### Scenario: Runtime manifest is written
- **WHEN** Custom Node preparation, overlay dependency installation, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL write a runtime manifest under the mounted workspace volume
- **AND** the manifest SHALL include the fixed image Python interpreter path, fixed image ComfyUI root path, workspace overlay path, preset-installed Custom Node revisions, required model asset paths, workspace overlay dependency records, and prepared timestamp
- **AND** the manifest MUST NOT include runtime contract id, runtime contract version, implementation revision, provisioner image identity, endpoint image identity, Python version, platform, ComfyUI revision, image base dependency record paths, runtime manifest compatibility metadata, or protected dependency policy version
- **AND** the manifest SHALL identify that the Python base runtime is image-baked and not materialized into the mounted workspace during provisioning

#### Scenario: Workspace dependency records are referenced
- **WHEN** workspace overlay dependency installation creates dependency records or install reports
- **THEN** the prepared runtime metadata SHALL reference only UI-safe workspace records needed for endpoint validation
- **AND** those records MUST NOT imply that base dependencies were installed during provisioning
- **AND** those records MUST NOT include provider API keys, worker bearer tokens, Hugging Face API keys, or other secrets

#### Scenario: Manifest is written after successful preparation
- **WHEN** overlay dependency installation fails, asset download fails, validation fails, or provisioning is cancelled
- **THEN** the Provisioner Worker MUST NOT write a terminal success runtime manifest for that workspace

### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL validate that the mounted prepared workspace contains the files needed to run generation with the fixed image-baked runtime environment.

#### Scenario: Runtime environment is valid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest declares required workspace-specific prepared paths
- **AND** the fixed image Python interpreter, fixed image ComfyUI entrypoint, required workflow file, required model file, required Custom Node paths, and declared overlay paths exist
- **THEN** the Endpoint Worker SHALL start ComfyUI through the fixed image-baked Python interpreter with the declared workspace overlay and workspace paths

#### Scenario: Runtime manifest is invalid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest is missing, invalid, or does not declare required workspace-specific prepared paths
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared runtime by installing dependencies

#### Scenario: Workspace environment is incomplete
- **WHEN** the Endpoint Worker starts generation
- **AND** the fixed image runtime, required workflow file, required model file, required Custom Node path, or declared overlay path is missing
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe missing environment error
- **AND** it MUST NOT run pip, clone repositories, create virtual environments, copy base runtime files, or download model assets to repair the workspace

### Requirement: Prepared runtime dependency records are workspace-resolved
The prepared runtime manifest SHALL advertise workspace dependency and install record paths that resolve under the mounted workspace.

#### Scenario: Manifest dependency records resolve under workspace
- **WHEN** the Provisioner Worker writes the prepared runtime manifest
- **THEN** every workspace dependency record, overlay install report, Custom Node record, and model asset record path SHALL resolve under the mounted workspace path
- **AND** each entry SHALL identify a file that exists before terminal success is reported

#### Scenario: Image base dependency records are not advertised
- **WHEN** the Provisioner Worker writes the prepared runtime manifest
- **THEN** it MUST NOT advertise image base dependency record paths
- **AND** the Endpoint Worker MUST NOT require image base dependency record paths to accept the prepared runtime manifest

### Requirement: Use image-baked base runtime with workspace overlay
The prepared workspace SHALL reference a fixed image-baked base runtime and SHALL contain only workspace-specific runtime data, dependency overlays, assets, outputs, and metadata.

#### Scenario: Provisioner uses image runtime
- **WHEN** the Provisioner Worker prepares a workspace
- **THEN** it SHALL use the fixed image Python interpreter and fixed image ComfyUI root
- **AND** it MUST NOT validate runtime identity metadata, catalog-declared image metadata, or base dependency records before writing terminal workspace metadata
- **AND** it MUST NOT extract, copy, or publish the base virtual environment or base ComfyUI tree into the mounted workspace volume

#### Scenario: Workspace-specific paths are prepared
- **WHEN** the Provisioner Worker prepares a workspace successfully
- **THEN** the mounted workspace SHALL contain workspace-specific model asset paths, Custom Node checkout paths, output paths, `.luma-forge` metadata paths, and `.luma-forge/python-overlay`
- **AND** the mounted workspace MUST NOT require `/workspace/.venv` or `/workspace/ComfyUI` to represent the deterministic base runtime

#### Scenario: Custom Node overlay is recorded
- **WHEN** Custom Node dependency installation succeeds
- **THEN** the prepared workspace SHALL record the overlay site-packages path, install report paths, and dependency records in the runtime manifest
- **AND** those records SHALL be sufficient for the Endpoint Worker to reproduce the import path configuration without running pip

### Requirement: Run ComfyUI through the image runtime and workspace overlay
The Endpoint Worker SHALL execute ComfyUI with the fixed image-baked Python interpreter and fixed image-baked ComfyUI root while adding workspace-specific Custom Node, model, output, and Python overlay paths.

#### Scenario: ComfyUI is started by endpoint worker
- **WHEN** the Endpoint Worker needs to start ComfyUI for a valid generation request
- **THEN** it SHALL execute the fixed image-baked ComfyUI entrypoint with the fixed image-baked Python interpreter
- **AND** it SHALL configure ComfyUI to use workspace model paths, workspace Custom Node paths, workspace output paths, and the workspace Python overlay declared by the prepared runtime manifest

#### Scenario: Workspace overlay augments image runtime
- **WHEN** the Endpoint Worker starts ComfyUI with a prepared workspace overlay
- **THEN** it SHALL add the declared overlay site-packages path to the ComfyUI process import path using fixed overlay-first precedence
- **AND** it MUST NOT install dependencies, run pip, or mutate the image-baked Python environment
