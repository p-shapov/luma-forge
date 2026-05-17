## MODIFIED Requirements

### Requirement: Prepare ComfyUI environment
The Provisioner Worker SHALL use the fixed image-baked ComfyUI runtime and SHALL prepare workspace-specific runtime directories without creating a base runtime copy on the mounted volume.

#### Scenario: ComfyUI runtime is prepared

- **WHEN** an active job contains a Workflow Preset and resolved runtime image snapshot accepted by the Native Layer
- **THEN** the Provisioner Worker SHALL use the fixed image-baked Python interpreter and fixed image-baked ComfyUI root
- **AND** the Provisioner Worker SHALL create or reuse workspace-specific directories for models, Custom Nodes, output, and `.luma-forge` metadata
- **AND** the Provisioner Worker SHALL reset and recreate `.luma-forge/python-overlay` plus stale Custom Node overlay install reports before installing Custom Node dependencies
- **AND** the Provisioner Worker MUST NOT clone ComfyUI, create a base virtual environment, extract a base runtime archive, or install ComfyUI base requirements during workspace provisioning
- **AND** `GET /status` SHALL report a preparation phase while this work is active

#### Scenario: ComfyUI preparation fails

- **WHEN** fixed image runtime access or workspace directory creation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include a UI-safe diagnostic message
- **AND** the diagnostic message MUST NOT include secrets

### Requirement: Validate prepared environment
The Provisioner Worker SHALL validate workspace-specific files, overlay metadata, and the runtime manifest before reporting terminal success.

#### Scenario: Prepared environment is valid

- **WHEN** required Custom Node directories, overlay dependency records, runtime manifest fields, model asset files, and workspace output paths are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the prepared workspace SHALL be usable by the Endpoint Worker with the fixed image-baked runtime environment

#### Scenario: Prepared environment is incomplete

- **WHEN** final validation finds a missing Custom Node, missing model asset, missing overlay record, missing runtime manifest data, missing workspace output path, or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success

### Requirement: Validate Provisioner Worker runtime environment
The Provisioner Worker SHALL validate its runtime environment before starting the HTTP server.

#### Scenario: Runtime environment is valid
- **WHEN** the provisioner process starts with a valid bearer token, bind host, bind port, request size limit, step timeouts, and workspace mount path
- **THEN** the Provisioner Worker SHALL start the HTTP server using the validated runtime configuration
- **AND** all worker modules SHALL use that same validated runtime configuration for authorization, request limits, step timeouts, and workspace mount validation

#### Scenario: Bearer token is missing or malformed
- **WHEN** `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` is missing, blank after trimming, shorter than 32 characters, or contains whitespace or control characters
- **THEN** the Provisioner Worker SHALL fail startup with a configuration error
- **AND** the Provisioner Worker MUST NOT start the HTTP server
- **AND** the startup failure SHALL emit machine-readable error code `configuration_error`
- **AND** the configuration error MUST NOT include the bearer token value

#### Scenario: Numeric runtime value is invalid
- **WHEN** `LUMA_FORGE_PROVISIONER_PORT`, `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES`, `LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS`, `LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS`, or `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS` is configured with a blank, non-numeric, non-finite, non-positive, or out-of-range value
- **THEN** the Provisioner Worker SHALL fail startup with a configuration error
- **AND** the startup failure SHALL emit machine-readable error code `configuration_error`
- **AND** the Provisioner Worker MUST NOT silently replace the configured value with a default
- **AND** the Provisioner Worker MUST NOT start the HTTP server

#### Scenario: Bind host is invalid
- **WHEN** `LUMA_FORGE_PROVISIONER_HOST` is configured with a blank value or a value that is not a valid IP address or DNS hostname
- **THEN** the Provisioner Worker SHALL fail startup with a configuration error
- **AND** the startup failure SHALL emit machine-readable error code `configuration_error`
- **AND** the Provisioner Worker MUST NOT start the HTTP server

#### Scenario: Workspace mount path is invalid
- **WHEN** `LUMA_FORGE_WORKSPACE_MOUNT_PATH` is configured with a blank, relative, or non-normalized path
- **THEN** the Provisioner Worker SHALL fail startup with a configuration error
- **AND** the startup failure SHALL emit machine-readable error code `configuration_error`
- **AND** the Provisioner Worker MUST NOT start the HTTP server

#### Scenario: Runtime configuration failure is machine-readable
- **WHEN** runtime environment validation fails during process startup
- **THEN** the Provisioner Worker SHALL write one structured diagnostic to stderr with code `configuration_error`
- **AND** the diagnostic SHALL include the affected environment variable name and a stable reason code
- **AND** the diagnostic MUST NOT include configured environment values or secrets
- **AND** the process SHALL exit before binding the HTTP server

## REMOVED Requirements

### Requirement: Validate resolved runtime contract before materialization
**Reason**: The Provisioner Worker no longer validates app runtime catalog implementation revisions, image identity, or image metadata before preparing a workspace.
**Migration**: Native Workspace Setup resolves the runtime contract id/version pair to immutable image refs, and the Provisioner Worker trusts the accepted start request plus fixed image-baked runtime layout.

### Requirement: Validate resolved runtime against provisioner image identity
**Reason**: `LUMA_FORGE_PROVISIONER_IMAGE_REF` and runtime implementation identity validation are removed.
**Migration**: Native creates the provisioning pod with the selected immutable image ref and injects only the bearer token needed for worker authorization.

### Requirement: Publish image-baked base runtime records
**Reason**: Base dependency record validation and catalog-declared image metadata are no longer part of runtime preparation.
**Migration**: Worker build and smoke tests remain responsible for ensuring the image contains the fixed base runtime; the prepared workspace manifest only needs workspace-specific records.
