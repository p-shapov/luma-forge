## ADDED Requirements

### Requirement: Expose Provisioner Worker HTTP API

The Provisioner Worker SHALL expose an HTTP API with `POST /start`, `POST /cancel`, and `GET /status` from inside the provisioner container.

#### Scenario: Worker starts idle

- **WHEN** the provisioner container starts
- **THEN** the Provisioner Worker SHALL start an HTTP server
- **AND** the Provisioner Worker SHALL report `idle` status before any `/start` request is accepted
- **AND** the Provisioner Worker MUST NOT prepare the ComfyUI environment before `/start`

#### Scenario: Worker API includes required endpoints

- **WHEN** a client calls the worker API
- **THEN** `POST /start`, `POST /cancel`, and `GET /status` SHALL be available
- **AND** the worker API MUST NOT expose Provider API Keys or Hugging Face API keys in any response

### Requirement: Start provisioning from selected Workflow Preset

The Provisioner Worker SHALL start one provisioning job only after `POST /start` receives a selected Workflow Preset payload and job correlation identifier.

#### Scenario: Start request is accepted

- **WHEN** `POST /start` receives a valid job identifier, selected Workflow Preset, and mounted workspace path while the worker is idle
- **THEN** the Provisioner Worker SHALL create one active provisioning job
- **AND** the Provisioner Worker SHALL begin preparing the mounted workspace volume
- **AND** `GET /status` SHALL report `running` with the active job identifier

#### Scenario: Start request is invalid

- **WHEN** `POST /start` receives a missing job identifier, missing selected Workflow Preset, unsupported source type, or unsafe install path
- **THEN** the Provisioner Worker SHALL reject the request
- **AND** the Provisioner Worker MUST remain idle
- **AND** the Provisioner Worker MUST NOT write to the mounted workspace volume

#### Scenario: Start request is concurrent

- **WHEN** `POST /start` is called while a provisioning job is active
- **THEN** the Provisioner Worker SHALL reject the request with a conflict error
- **AND** the Provisioner Worker MUST NOT start, queue, or replace a second job
- **AND** the active job SHALL continue unless separately cancelled

### Requirement: Prepare ComfyUI environment

The Provisioner Worker SHALL prepare the mounted workspace volume by installing the ComfyUI runtime declared by the selected Workflow Preset.

#### Scenario: ComfyUI runtime is prepared

- **WHEN** an active job contains a Workflow Preset with a supported Git ComfyUI runtime source
- **THEN** the Provisioner Worker SHALL clone or update ComfyUI into the mounted workspace volume
- **AND** the Provisioner Worker SHALL install ComfyUI dependencies required by that runtime
- **AND** `GET /status` SHALL report an installation phase while this work is active

#### Scenario: ComfyUI preparation fails

- **WHEN** the ComfyUI Git checkout or dependency installation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include a UI-safe diagnostic message
- **AND** the diagnostic message MUST NOT include secrets

### Requirement: Prepare Custom Nodes

The Provisioner Worker SHALL install required Custom Nodes declared by the selected Workflow Preset.

#### Scenario: Preset declares Custom Nodes

- **WHEN** the selected Workflow Preset includes required Custom Nodes
- **THEN** the Provisioner Worker SHALL clone each Custom Node from its declared Git source
- **AND** the Provisioner Worker SHALL install Custom Node dependencies from each declared requirements path when present
- **AND** `GET /status` SHALL report an installing Custom Nodes phase while this work is active

#### Scenario: Preset declares no Custom Nodes

- **WHEN** the selected Workflow Preset has an empty required Custom Nodes list
- **THEN** the Provisioner Worker SHALL skip Custom Node installation
- **AND** the provisioning job SHALL continue to the next required preparation step

### Requirement: Download public Hugging Face model assets

The Provisioner Worker SHALL download required model assets from public Hugging Face sources declared by the selected Workflow Preset.

#### Scenario: Public Hugging Face asset is downloaded

- **WHEN** the selected Workflow Preset declares a Hugging Face model asset with repository id, file path, revision, and explicit install path
- **THEN** the Provisioner Worker SHALL download the public file from Hugging Face
- **AND** the Provisioner Worker SHALL write it to the declared ComfyUI-relative install path under the prepared ComfyUI root
- **AND** `GET /status` SHALL report a downloading assets phase while asset downloads are active

#### Scenario: Hugging Face asset requires authentication

- **WHEN** Hugging Face rejects a model asset download because authentication or authorization is required
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** the Provisioner Worker MUST NOT request, read, persist, or log a Hugging Face API key

#### Scenario: Asset install path is unsafe

- **WHEN** a model asset install path is absolute, blank, contains parent traversal, or resolves outside the prepared ComfyUI root
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before writing that asset
- **AND** the Provisioner Worker MUST NOT write outside the prepared ComfyUI root

### Requirement: Report provisioning status

The Provisioner Worker SHALL report UI-safe provisioning job status through `GET /status`.

#### Scenario: Job is running

- **WHEN** a provisioning job is active
- **THEN** `GET /status` SHALL return the active job identifier, status `running`, current phase, updated timestamp, and optional progress percentage
- **AND** the response MAY include a UI-safe diagnostic message

#### Scenario: Job succeeds

- **WHEN** ComfyUI, Custom Nodes, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL mark the job `succeeded`
- **AND** `GET /status` SHALL report terminal success

#### Scenario: Job fails

- **WHEN** a provisioning step cannot complete safely
- **THEN** the Provisioner Worker SHALL mark the job `failed`
- **AND** `GET /status` SHALL report terminal failure with UI-safe error metadata
- **AND** the response MUST NOT include provider secrets, tokens, or raw credential-bearing command output

### Requirement: Cancel active provisioning

The Provisioner Worker SHALL support cancellation of the active provisioning job.

#### Scenario: Active job is cancelled

- **WHEN** `POST /cancel` is called with the active job identifier while provisioning work is active
- **THEN** the Provisioner Worker SHALL request cancellation of the active job
- **AND** `GET /status` SHALL report `cancelling` until active work has stopped
- **AND** the Provisioner Worker SHALL report `cancelled` after cancellation completes

#### Scenario: Cancel request has no matching active job

- **WHEN** `POST /cancel` is called while the worker has no matching active job
- **THEN** the Provisioner Worker SHALL reject the cancellation request
- **AND** the Provisioner Worker MUST NOT change terminal job status solely because of the unmatched cancellation request

### Requirement: Validate prepared environment

The Provisioner Worker SHALL validate the prepared ComfyUI environment before reporting terminal success.

#### Scenario: Prepared environment is valid

- **WHEN** all required ComfyUI files, Custom Node directories, dependency installs, and model asset files are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the prepared environment SHALL be usable by the future Endpoint Worker as a mounted runtime environment

#### Scenario: Prepared environment is incomplete

- **WHEN** final validation finds a missing required file, missing Custom Node, missing model asset, or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success
