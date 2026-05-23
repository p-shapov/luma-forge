# Provisioner Worker Specification

## Purpose
Define the Provisioner Worker API and workspace preparation responsibilities.
## Requirements
### Requirement: Prepare workspace paths without endpoint runtime
The Provisioner Worker SHALL prepare workspace-specific directories without requiring, validating, or starting a ComfyUI runtime and without writing endpoint runtime metadata on the mounted volume.

#### Scenario: Runtime paths are validated and prepared
- **WHEN** an active job contains a worker start request accepted by the Provisioner Worker
- **THEN** the Provisioner Worker SHALL create or reuse workspace-specific directories required for model downloads and provisioning operation
- **AND** the Provisioner Worker MUST NOT require endpoint image fields in the start request
- **AND** the Provisioner Worker MUST NOT require Workflow Preset id, Workflow Preset version, workflow execution type, required base volume size, runtime contract reference, provisioner contract reference, resolved runtime image snapshot, resolved provisioner image snapshot, or model asset kind in the start request
- **AND** the Provisioner Worker MUST NOT start ComfyUI, clone ComfyUI, create a base virtual environment, extract a base runtime archive, run `comfy install`, run pip, clone runtime extensions, install runtime extension dependencies, install ComfyUI base requirements, or write `.luma-forge/runtime-manifest.json` during workspace provisioning
- **AND** `GET /status` SHALL report a preparation phase while this work is active
- **AND** current worker status payloads MUST NOT emit obsolete ComfyUI installation phase names such as `installing_comfyui`

#### Scenario: Workspace path preparation fails
- **WHEN** workspace directory creation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include status, phase, progress percentage, and structured error metadata when available
- **AND** the status payload MUST NOT include secrets

### Requirement: Report provisioning status
The Provisioner Worker SHALL report UI-safe provisioning job status through `GET /status`.

#### Scenario: No job has started
- **WHEN** no job has started since worker startup
- **THEN** `GET /status` SHALL return `idle`
- **AND** the response MAY include no active job id
- **AND** the response MAY include no active phase
- **AND** the response SHALL NOT include an error

#### Scenario: Job is running
- **WHEN** an accepted job is actively preparing the workspace
- **THEN** `GET /status` SHALL return `running`
- **AND** the response SHALL include the active job id
- **AND** the response SHALL include a UI-safe phase value
- **AND** the response MAY include bounded progress percent
- **AND** the response SHALL NOT include raw command output, raw provider responses, bearer tokens, Provider API Keys, Hugging Face keys, or credential-bearing URLs
- **AND** the response MAY include structured error metadata

#### Scenario: Job succeeds
- **WHEN** required workspace preparation, model asset downloads, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL mark the job `succeeded`
- **AND** `GET /status` SHALL report terminal success
- **AND** the terminal success response MAY report no active phase

#### Scenario: Job fails
- **WHEN** a provisioning step cannot complete safely
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL report terminal failure with UI-safe error metadata
- **AND** the terminal error metadata SHALL use the standard worker error payload shape with `code` and `message`
- **AND** the terminal error `code` SHALL be the stable specific worker error classifier
- **AND** the terminal error metadata MUST NOT include `reason_code`
- **AND** the response MAY include structured error metadata
- **AND** the response MUST NOT include provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

### Requirement: Validate prepared environment
The Provisioner Worker SHALL validate workspace-specific model files and path safety before reporting terminal success.

#### Scenario: Prepared environment is valid
- **WHEN** model asset files declared by the selected Workflow Preset are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the Provisioner Worker MUST NOT require workflow paths, output paths, or runtime manifest data to report terminal success

#### Scenario: Prepared environment is incomplete
- **WHEN** final validation finds a missing declared model asset or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success

### Requirement: Configure workspace mount path from environment
The Provisioner Worker SHALL treat `LUMA_FORGE_WORKSPACE_MOUNT_PATH` as the workspace mount path configured by Native provisioning.

#### Scenario: Provisioner Worker receives workspace mount path
- **WHEN** the Provisioner Worker container starts with `LUMA_FORGE_WORKSPACE_MOUNT_PATH` set to an absolute safe path
- **THEN** the Provisioner Worker SHALL use that path as the workspace preparation root
- **AND** model asset installation and workspace directory preparation SHALL occur under that path

#### Scenario: Provisioner Worker receives invalid workspace mount path
- **WHEN** the Provisioner Worker container starts with `LUMA_FORGE_WORKSPACE_MOUNT_PATH` set to an empty, relative, or unsafe path
- **THEN** the Provisioner Worker SHALL reject the configuration
- **AND** it MUST NOT prepare files under a fallback path for that invalid configuration

