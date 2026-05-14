## MODIFIED Requirements

### Requirement: Prepare ComfyUI environment

The Provisioner Worker SHALL prepare the mounted workspace volume by installing the ComfyUI runtime declared by the selected Workflow Preset and by installing ComfyUI Python dependencies into a volume-local virtual environment.

#### Scenario: ComfyUI runtime is prepared

- **WHEN** an active job contains a Workflow Preset with a supported Git ComfyUI runtime source
- **THEN** the Provisioner Worker SHALL clone or update ComfyUI into the mounted workspace volume
- **AND** the Provisioner Worker SHALL create or reuse a virtual environment under the mounted workspace volume
- **AND** the Provisioner Worker SHALL install ComfyUI dependencies required by that runtime through the volume-local virtual environment interpreter
- **AND** the Provisioner Worker MUST NOT install ComfyUI runtime dependencies into the ephemeral provisioner container Python environment
- **AND** `GET /status` SHALL report an installation phase while this work is active

#### Scenario: ComfyUI preparation fails

- **WHEN** the ComfyUI Git checkout, volume-local virtual environment creation, or dependency installation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include a UI-safe diagnostic message
- **AND** the diagnostic message MUST NOT include secrets

### Requirement: Prepare Custom Nodes

The Provisioner Worker SHALL install required Custom Nodes declared by the selected Workflow Preset and SHALL install their Python dependencies into the prepared volume-local virtual environment.

#### Scenario: Preset declares Custom Nodes

- **WHEN** the selected Workflow Preset includes required Custom Nodes
- **THEN** the Provisioner Worker SHALL clone each Custom Node from its declared Git source into its declared safe checkout path under the prepared ComfyUI `custom_nodes` directory
- **AND** the Provisioner Worker SHALL install Custom Node dependencies from each declared requirements path into the volume-local virtual environment when present
- **AND** each requirements path SHALL be resolved relative to its Custom Node checkout root
- **AND** the Provisioner Worker MUST NOT install Custom Node dependencies into the ephemeral provisioner container Python environment
- **AND** `GET /status` SHALL report an installing Custom Nodes phase while this work is active

#### Scenario: Preset declares no Custom Nodes

- **WHEN** the selected Workflow Preset has an empty required Custom Nodes list
- **THEN** the Provisioner Worker SHALL skip Custom Node installation
- **AND** the provisioning job SHALL continue to the next required preparation step

### Requirement: Validate prepared environment

The Provisioner Worker SHALL validate the prepared ComfyUI environment and volume-local virtual environment before reporting terminal success.

#### Scenario: Prepared environment is valid

- **WHEN** all required ComfyUI files, Custom Node directories, dependency records, runtime manifest fields, volume-local virtual environment files, and model asset files are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the prepared environment SHALL be usable by the future Endpoint Worker as a mounted runtime environment

#### Scenario: Prepared environment is incomplete

- **WHEN** final validation finds a missing required file, missing Custom Node, missing model asset, missing volume-local virtual environment interpreter, missing runtime manifest data, or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success

### Requirement: Report structured worker error codes
The Provisioner Worker SHALL map expected failure classes to stable UI-safe error codes and messages.

#### Scenario: Git checkout fails
- **WHEN** a ComfyUI or Custom Node Git clone, fetch, or checkout operation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `git_checkout_failed`
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Dependency installation fails
- **WHEN** ComfyUI or Custom Node dependency installation into the volume-local virtual environment fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `dependency_install_failed`
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Volume environment creation fails
- **WHEN** the Provisioner Worker cannot create or validate the volume-local virtual environment
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `dependency_install_failed`
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Model download fails
- **WHEN** a public Hugging Face model asset cannot be downloaded because of transport, missing file, or unavailable service failure
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_download_failed`

#### Scenario: Model download requires authentication
- **WHEN** Hugging Face rejects a model asset download because authentication or authorization is required
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_auth_required`
- **AND** the Provisioner Worker MUST NOT request, read, persist, or log a Hugging Face API key

#### Scenario: Path validation fails
- **WHEN** a request contains an unsafe workspace, Custom Node, requirements, virtual environment, runtime metadata, or model asset install path
- **THEN** the Provisioner Worker SHALL reject the start request or mark the active job `failed` before the unsafe write or read
- **AND** the API response or job status SHALL include error code `path_validation_failed`

#### Scenario: Provisioning step times out
- **WHEN** a Git, virtual environment creation, dependency installation, or model download step exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the active operation where possible
- **AND** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `step_timeout`
