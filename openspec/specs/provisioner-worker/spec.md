# provisioner-worker Specification

## Purpose
TBD - created by archiving change add-provisioner-worker. Update Purpose after archive.
## Requirements
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

- **WHEN** `POST /start` receives a valid job identifier and selected Workflow Preset while the worker is idle
- **THEN** the Provisioner Worker SHALL create one active provisioning job
- **AND** the Provisioner Worker SHALL begin preparing the configured mounted workspace volume
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

### Requirement: Download public Hugging Face model assets
The Provisioner Worker SHALL download required model assets from public Hugging Face sources declared by the selected Workflow Preset using Hugging Face Hub download and cache semantics.

#### Scenario: Public Hugging Face asset is downloaded
- **WHEN** the selected Workflow Preset declares a Hugging Face model asset with repository id, file path, revision, and explicit install path
- **THEN** the Provisioner Worker SHALL download the public file from Hugging Face using the declared repository id, file path, and revision
- **AND** the Provisioner Worker SHALL write it to the declared ComfyUI-relative install path under the prepared ComfyUI root
- **AND** the Provisioner Worker SHALL rely on Hugging Face Hub cache semantics for download reuse
- **AND** the Provisioner Worker MUST NOT require or validate an app-owned digest for the asset
- **AND** `GET /status` SHALL report a downloading assets phase while asset downloads are active

#### Scenario: Hugging Face Hub cache can satisfy an asset
- **WHEN** Hugging Face Hub reports that the declared repository id, file path, and revision are already cached or up to date
- **THEN** the Provisioner Worker MAY use the Hub-provided cached file instead of re-downloading the asset
- **AND** the Provisioner Worker MUST NOT decide cache reuse outside Hugging Face Hub cache semantics

#### Scenario: Hugging Face asset requires authentication
- **WHEN** Hugging Face rejects a model asset download because authentication or authorization is required
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_auth_required`
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
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL report terminal failure with UI-safe error metadata
- **AND** the terminal error metadata SHALL use the standard worker error payload shape with `code`, `reason_code`, and `message`
- **AND** the response MUST NOT include provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

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

The Provisioner Worker SHALL validate the prepared ComfyUI environment and volume-local virtual environment before reporting terminal success.

#### Scenario: Prepared environment is valid

- **WHEN** all required ComfyUI files, Custom Node directories, dependency records, runtime manifest fields, volume-local virtual environment files, and model asset files are present after preparation
- **THEN** the Provisioner Worker SHALL report the job as `succeeded`
- **AND** the prepared environment SHALL be usable by the future Endpoint Worker as a mounted runtime environment

#### Scenario: Prepared environment is incomplete

- **WHEN** final validation finds a missing required file, missing Custom Node, missing model asset, missing volume-local virtual environment interpreter, missing runtime manifest data, or unsafe filesystem state
- **THEN** the Provisioner Worker SHALL report the job as `failed`
- **AND** the Provisioner Worker MUST NOT report terminal success

### Requirement: Validate Custom Node paths before filesystem writes

The Provisioner Worker SHALL validate Custom Node checkout and requirements paths from the selected Workflow Preset before performing related remote filesystem writes or dependency installation.

#### Scenario: Custom Node checkout path is safe

- **WHEN** a selected Workflow Preset declares a Custom Node checkout path that is relative, normalized, free of current-directory, empty, absolute, and parent-traversal segments, and resolves under the prepared ComfyUI `custom_nodes` directory
- **THEN** the Provisioner Worker MAY clone that Custom Node into the resolved checkout path

#### Scenario: Custom Node checkout path is unsafe

- **WHEN** a selected Workflow Preset declares a Custom Node checkout path that is blank, absolute, contains current-directory, empty, or parent-traversal segments, resolves outside the prepared ComfyUI root, or does not resolve under the prepared ComfyUI `custom_nodes` directory
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before cloning the Custom Node
- **AND** the Provisioner Worker MUST NOT write outside the prepared ComfyUI `custom_nodes` directory for that Custom Node

#### Scenario: Custom Node requirements path is absent

- **WHEN** a selected Workflow Preset declares no requirements path for a Custom Node
- **THEN** the Provisioner Worker SHALL skip requirements installation for that Custom Node
- **AND** the Provisioner Worker SHALL continue provisioning when all other Custom Node data is valid

#### Scenario: Custom Node requirements path is safe

- **WHEN** a selected Workflow Preset declares a Custom Node requirements path that is relative, normalized, free of current-directory, empty, absolute, and parent-traversal segments, and resolves under that Custom Node checkout root
- **THEN** the Provisioner Worker MAY install dependencies from that requirements path

#### Scenario: Custom Node requirements path is unsafe

- **WHEN** a selected Workflow Preset declares a Custom Node requirements path that is blank, absolute, contains current-directory, empty, or parent-traversal segments, or resolves outside that Custom Node checkout root
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before installing dependencies from that path
- **AND** the Provisioner Worker MUST NOT read requirements files outside the Custom Node checkout root

### Requirement: Authorize worker API requests
The Provisioner Worker SHALL require bearer-token authorization for every HTTP endpoint.

#### Scenario: Authorized request is accepted
- **WHEN** the client calls `GET /status`, `POST /start`, or `POST /cancel` with `Authorization: Bearer <configured-token>`
- **THEN** the Provisioner Worker SHALL process the request normally

#### Scenario: Unauthorized request is rejected
- **WHEN** the client omits the authorization header or provides a different token
- **THEN** the Provisioner Worker SHALL reject the request with `unauthorized`
- **AND** the Provisioner Worker MUST NOT start, cancel, or expose any provisioning job state mutation because of that request
- **AND** the response MUST NOT include the configured token

### Requirement: Bound worker request bodies
The Provisioner Worker SHALL enforce a configured maximum request body size before decoding request JSON.

#### Scenario: Request body is within the limit
- **WHEN** `POST /start` or `POST /cancel` includes a valid `Content-Length` that is less than or equal to the configured maximum
- **THEN** the Provisioner Worker MAY read and parse the request body

#### Scenario: Request body is too large
- **WHEN** `POST /start` or `POST /cancel` includes a `Content-Length` greater than the configured maximum
- **THEN** the Provisioner Worker SHALL reject the request with `request_too_large`
- **AND** the Provisioner Worker MUST NOT read the oversized body into memory
- **AND** the Provisioner Worker MUST NOT mutate provisioning job state

#### Scenario: Request body length is malformed
- **WHEN** `POST /start` or `POST /cancel` includes a missing, negative, or non-integer `Content-Length`
- **THEN** the Provisioner Worker SHALL reject the request with `invalid_request`
- **AND** the Provisioner Worker MUST NOT mutate provisioning job state

### Requirement: Report structured worker error codes
The Provisioner Worker SHALL map expected failure classes to stable UI-safe error codes, reason codes, and messages.

#### Scenario: Git checkout fails
- **WHEN** a ComfyUI or Custom Node Git clone, fetch, or checkout operation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `git_checkout_failed`
- **AND** `GET /status` SHALL include a stable `reason_code` for the Git checkout failure
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Dependency installation fails
- **WHEN** ComfyUI or Custom Node dependency installation into the volume-local virtual environment fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `dependency_install_failed`
- **AND** `GET /status` SHALL include a stable `reason_code` for the dependency installation failure
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Volume environment creation fails
- **WHEN** the Provisioner Worker cannot create or validate the volume-local virtual environment
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `dependency_install_failed`
- **AND** `GET /status` SHALL include a stable `reason_code` for the volume environment failure
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Model download fails
- **WHEN** a public Hugging Face model asset cannot be downloaded because of transport, missing file, or unavailable service failure
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_download_failed`
- **AND** `GET /status` SHALL include a stable `reason_code` for the asset download failure

#### Scenario: Model download requires authentication
- **WHEN** Hugging Face rejects a model asset download because authentication or authorization is required
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `asset_auth_required`
- **AND** `GET /status` SHALL include a stable `reason_code` for the asset authorization failure
- **AND** the Provisioner Worker MUST NOT request, read, persist, or log a Hugging Face API key

#### Scenario: Path validation fails
- **WHEN** a request contains an unsafe workspace, Custom Node, requirements, virtual environment, runtime metadata, or model asset install path
- **THEN** the Provisioner Worker SHALL reject the start request or mark the active job `failed` before the unsafe write or read
- **AND** the API response or job status SHALL include error code `path_validation_failed`
- **AND** the API response or job status SHALL include a stable `reason_code` for the path validation failure
- **AND** any exposed context MUST include only safe field or path-role identifiers, not the raw unsafe path value

#### Scenario: Provisioning step times out
- **WHEN** a Git, virtual environment creation, dependency installation, or model download step exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the active operation where possible
- **AND** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `step_timeout`
- **AND** `GET /status` SHALL include a stable `reason_code` for the timed-out step

#### Scenario: Request validation fails
- **WHEN** a worker API request is malformed, has invalid JSON, has invalid content length, or fails payload validation
- **THEN** the worker API response SHALL include an error code such as `invalid_request` or `invalid_json`
- **AND** the worker API response SHALL include a stable `reason_code` for the request validation failure

#### Scenario: Request is unauthorized
- **WHEN** the client omits authorization or provides invalid authorization
- **THEN** the worker API response SHALL include error code `unauthorized`
- **AND** the worker API response SHALL include a stable `reason_code` for the authorization failure
- **AND** the response MUST NOT reveal whether the supplied token matched any prefix, length, or token format rule

#### Scenario: Request conflicts with active job
- **WHEN** `POST /start` is called while a provisioning job is active
- **THEN** the worker API response SHALL include error code `job_already_running`
- **AND** the worker API response SHALL include a stable `reason_code` for the active job conflict
- **AND** the active job identifier MAY be included only as UI-safe structured context

### Requirement: Require immutable Git revisions for worker-prepared sources
The Provisioner Worker SHALL reject worker-prepared Git sources that do not specify immutable commit revisions.

#### Scenario: Git source revision is immutable
- **WHEN** a selected Workflow Preset declares ComfyUI or Custom Node Git sources with full 40-character lowercase hexadecimal commit revisions
- **THEN** the Provisioner Worker MAY use those revisions for checkout

#### Scenario: Git source revision is mutable
- **WHEN** a selected Workflow Preset declares a ComfyUI or Custom Node Git source revision as a branch name, tag name, blank value, or non-commit value
- **THEN** the Provisioner Worker SHALL reject the start request with `invalid_request`
- **AND** the Provisioner Worker MUST NOT clone, fetch, checkout, or install dependencies for that request

### Requirement: Bound external provisioning steps
The Provisioner Worker SHALL apply configured timeouts to external Git, virtual environment creation, dependency installation, and model download work.

#### Scenario: External step completes before timeout
- **WHEN** a Git command, virtual environment creation, dependency installation, or model download completes before its configured timeout
- **THEN** the Provisioner Worker SHALL continue provisioning normally

#### Scenario: External step exceeds timeout
- **WHEN** a Git command, virtual environment creation, dependency installation, or model download exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the operation where possible
- **AND** the Provisioner Worker SHALL fail the active job with `step_timeout`

### Requirement: Format worker error payloads consistently
The Provisioner Worker SHALL serialize runtime worker errors with a consistent UI-safe error payload shape.

#### Scenario: Immediate worker API error is returned
- **WHEN** `GET /status`, `POST /start`, or `POST /cancel` rejects a request before starting or changing provisioning work
- **THEN** the HTTP response body SHALL include `code`, `reason_code`, and `message`
- **AND** `code` SHALL remain the broad stable worker error classification
- **AND** `reason_code` SHALL identify the specific stable reason within that classification
- **AND** the response MAY include `context` containing only allowlisted UI-safe metadata
- **AND** the response MUST NOT include bearer tokens, provider API keys, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

#### Scenario: Terminal job error is returned
- **WHEN** `GET /status` reports a failed provisioning job
- **THEN** the `error` object SHALL include `code`, `reason_code`, and `message`
- **AND** the `error` object MAY include `context` containing only allowlisted UI-safe metadata
- **AND** the `error` object MUST NOT include bearer tokens, provider API keys, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

#### Scenario: Worker error has no additional context
- **WHEN** a worker error has no safe metadata to expose
- **THEN** the serialized error payload SHALL omit `context`
- **AND** consumers MUST NOT need to parse `message` to classify the error

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
