## ADDED Requirements

### Requirement: Prepare a volume-local Python runtime
The prepared workspace SHALL include a Python virtual environment stored on the mounted network volume for ComfyUI runtime dependencies.

#### Scenario: Volume virtual environment is created
- **WHEN** the Provisioner Worker prepares a workspace
- **THEN** it SHALL create or reuse a Python virtual environment under the mounted workspace volume
- **AND** the virtual environment path SHALL be stable for the workspace
- **AND** the virtual environment MUST NOT be located in the provisioner container's ephemeral application directory

#### Scenario: ComfyUI dependencies are installed into the volume environment
- **WHEN** the Provisioner Worker installs ComfyUI Python dependencies
- **THEN** it SHALL invoke pip through the volume-local virtual environment interpreter
- **AND** it MUST NOT invoke pip through the provisioner container's default Python environment for ComfyUI runtime dependencies

#### Scenario: Custom Node dependencies are installed into the volume environment
- **WHEN** a selected Workflow Preset declares a Custom Node requirements file
- **THEN** the Provisioner Worker SHALL install those requirements through the volume-local virtual environment interpreter
- **AND** it MUST NOT install those requirements into the provisioner container's default Python environment

### Requirement: Record prepared runtime metadata
The prepared workspace SHALL include runtime metadata that describes the volume-local Python runtime and the resolved dependency environment.

#### Scenario: Runtime manifest is written
- **WHEN** ComfyUI, Custom Nodes, model assets, and dependency installation complete successfully
- **THEN** the Provisioner Worker SHALL write a runtime manifest under the mounted workspace volume
- **AND** the manifest SHALL include the environment kind, Python interpreter path, ComfyUI root path, Python version, platform, ComfyUI revision, and prepared timestamp

#### Scenario: Dependency resolution records are written
- **WHEN** Python dependency installation completes successfully
- **THEN** the Provisioner Worker SHALL write a frozen package record under the mounted workspace volume
- **AND** it SHALL write a pip install report when the configured pip version supports install reports
- **AND** these records MUST NOT include provider API keys, worker bearer tokens, or other secrets

#### Scenario: Manifest is written after successful preparation
- **WHEN** dependency installation fails, validation fails, or provisioning is cancelled
- **THEN** the Provisioner Worker MUST NOT write a terminal success runtime manifest for that workspace

### Requirement: Validate endpoint runtime environment
The Endpoint Worker SHALL validate that the mounted prepared runtime environment has the required volume-local runtime shape before running generation.

#### Scenario: Runtime environment is valid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest declares a volume-local virtual environment
- **AND** the volume-local Python interpreter and ComfyUI entrypoint exist
- **THEN** the Endpoint Worker MAY start ComfyUI through the volume-local Python interpreter

#### Scenario: Runtime manifest is invalid
- **WHEN** the Endpoint Worker starts generation
- **AND** the prepared runtime manifest is missing, invalid, or does not declare a volume-local virtual environment
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared runtime by installing dependencies

#### Scenario: Volume environment is incomplete
- **WHEN** the Endpoint Worker starts generation
- **AND** the volume-local Python interpreter, ComfyUI entrypoint, required workflow file, required model file, or required Custom Node path is missing
- **THEN** the Endpoint Worker SHALL fail the request with a stable UI-safe missing environment error
- **AND** it MUST NOT run pip, clone repositories, or download model assets to repair the workspace

### Requirement: Run ComfyUI through the prepared volume environment
The Endpoint Worker SHALL execute ComfyUI with the Python interpreter from the prepared workspace virtual environment.

#### Scenario: ComfyUI is started by endpoint worker
- **WHEN** the Endpoint Worker needs to start ComfyUI for a valid generation request
- **THEN** it SHALL execute the ComfyUI entrypoint with the Python interpreter declared by the prepared runtime manifest
- **AND** it SHALL use the prepared ComfyUI root from the mounted workspace volume

#### Scenario: Endpoint container Python is not used for ComfyUI
- **WHEN** the Endpoint Worker executes ComfyUI
- **THEN** it MUST NOT execute ComfyUI through the endpoint container's default Python environment unless that interpreter is the same volume-local interpreter declared by the runtime manifest
