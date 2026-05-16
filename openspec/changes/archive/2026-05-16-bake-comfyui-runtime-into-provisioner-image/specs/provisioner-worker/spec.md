## MODIFIED Requirements

### Requirement: Prepare ComfyUI environment
The Provisioner Worker SHALL prepare the mounted workspace volume by extracting the Docker-build-produced ComfyUI runtime archive and verifying the materialized runtime.

#### Scenario: ComfyUI runtime is prepared
- **WHEN** an active job references a resolved runtime contract implementation compatible with the Provisioner Worker image
- **THEN** the Provisioner Worker SHALL extract the image-baked runtime archive into a staging path under the mounted workspace volume
- **AND** the Provisioner Worker SHALL publish ComfyUI into the mounted workspace volume from the staged runtime only after extraction validation succeeds
- **AND** the Provisioner Worker SHALL materialize the prebuilt Python virtual environment into the mounted workspace volume
- **AND** the Provisioner Worker MUST NOT clone or update ComfyUI from Git during provisioning
- **AND** the Provisioner Worker MUST NOT create a fresh virtual environment during provisioning
- **AND** the Provisioner Worker MUST NOT install base ComfyUI runtime dependencies during provisioning
- **AND** `GET /status` SHALL report a runtime materialization or environment preparation phase while this work is active

#### Scenario: ComfyUI preparation fails
- **WHEN** runtime archive materialization, prepared runtime verification, or workspace path validation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include a UI-safe diagnostic message
- **AND** the diagnostic message MUST NOT include secrets

### Requirement: Prepare Custom Nodes
The Provisioner Worker SHALL install or verify only the Custom Nodes required by the selected Workflow Preset after the image-baked base runtime archive has been materialized.

#### Scenario: Preset declares Custom Nodes
- **WHEN** the selected Workflow Preset includes required Custom Nodes
- **THEN** the Provisioner Worker SHALL install or verify each required Custom Node at its declared safe path under the materialized ComfyUI `custom_nodes` directory
- **AND** the Provisioner Worker SHALL use the Custom Node sources, revisions, install paths, and dependency declarations from the selected Workflow Preset
- **AND** the Provisioner Worker MAY install preset-declared Custom Node dependencies into the materialized `/workspace/.venv`
- **AND** the Provisioner Worker MUST NOT install Custom Nodes that are not declared by the selected Workflow Preset
- **AND** the Provisioner Worker MUST NOT select Custom Node dependencies from GPU placement data
- **AND** `GET /status` SHALL report an environment preparation or validation phase while this work is active

#### Scenario: Preset declares no Custom Nodes
- **WHEN** the selected Workflow Preset has an empty required Custom Nodes list
- **THEN** the Provisioner Worker SHALL skip Custom Node verification for preset-required nodes
- **AND** the provisioning job SHALL continue to the next required preparation step

### Requirement: Download public Hugging Face model assets
The Provisioner Worker SHALL download required model assets from public Hugging Face sources declared by the selected Workflow Preset using Hugging Face Hub download and cache semantics.

#### Scenario: Public Hugging Face asset is downloaded
- **WHEN** the selected Workflow Preset declares a Hugging Face model asset with repository id, file path, revision, and explicit install path
- **THEN** the Provisioner Worker SHALL download the public file from Hugging Face using the declared repository id, file path, and revision
- **AND** the Provisioner Worker SHALL write it to the declared ComfyUI-relative install path under the materialized ComfyUI root
- **AND** the Provisioner Worker SHALL rely on Hugging Face Hub cache semantics for download reuse
- **AND** the Provisioner Worker MUST NOT require or validate an app-owned digest for the asset
- **AND** `GET /status` SHALL report a downloading assets phase while asset downloads are active

#### Scenario: Hugging Face Hub cache can satisfy an asset
- **WHEN** Hugging Face Hub reports that the declared repository id, file path, and revision are already cached or up to date
- **THEN** the Provisioner Worker SHALL use the Hub-provided cached file instead of re-downloading the asset
- **AND** the Provisioner Worker MUST NOT decide cache reuse outside Hugging Face Hub cache semantics

#### Scenario: Hugging Face asset requires authentication
- **WHEN** Hugging Face rejects a model asset download because authentication or authorization is required
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_auth_required`
- **AND** the Provisioner Worker MUST NOT request, read, persist, or log a Hugging Face API key

#### Scenario: Asset install path is unsafe
- **WHEN** a model asset install path is absolute, blank, contains parent traversal, or resolves outside the materialized ComfyUI root
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before writing that asset
- **AND** the Provisioner Worker MUST NOT write outside the materialized ComfyUI root

### Requirement: Validate prepared environment
The Provisioner Worker SHALL validate the materialized ComfyUI environment and materialized volume-local virtual environment before reporting terminal success.

#### Scenario: Prepared environment is valid
- **WHEN** all required ComfyUI files, preset-required Custom Node directories, runtime contract metadata, materialized virtual environment files, runtime manifest fields, and model asset files are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the prepared environment SHALL be usable by the Endpoint Worker as a mounted runtime environment

#### Scenario: Prepared environment is incomplete
- **WHEN** final validation finds a missing required file, missing Custom Node, missing model asset, missing materialized virtual environment interpreter, missing runtime manifest data, incompatible runtime contract, or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success

## ADDED Requirements

### Requirement: Validate resolved runtime contract before materialization
The Provisioner Worker SHALL reject preparation requests whose resolved runtime contract or implementation revision does not match the runtime contract implementation declared by the Provisioner Worker image.

#### Scenario: Resolved runtime contract matches image
- **WHEN** a start request includes a resolved runtime contract id, version, implementation revision, and image identity matching the Provisioner Worker image runtime metadata
- **THEN** the Provisioner Worker SHALL accept the runtime contract implementation for materialization
- **AND** it MUST NOT perform a ComfyUI Git checkout during provisioning

#### Scenario: Resolved runtime contract is missing or mismatched
- **WHEN** a start request omits resolved runtime contract metadata or declares a runtime contract id, version, implementation revision, or image identity that does not match the Provisioner Worker image runtime metadata
- **THEN** the Provisioner Worker SHALL reject the start request with `invalid_request`
- **AND** the Provisioner Worker MUST NOT clone, fetch, checkout, create virtual environments, or install dependencies for that request

## MODIFIED Requirements

### Requirement: Bound external provisioning steps
The Provisioner Worker SHALL apply configured timeouts to external runtime materialization, Custom Node preparation, runtime verification, and model download work.

#### Scenario: External step completes before timeout
- **WHEN** runtime materialization, Custom Node preparation, runtime verification, or model download completes before its configured timeout
- **THEN** the Provisioner Worker SHALL continue provisioning normally

#### Scenario: External step exceeds timeout
- **WHEN** runtime materialization, Custom Node preparation, runtime verification, or model download exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the operation where possible
- **AND** the Provisioner Worker SHALL fail the active job with `step_timeout`
