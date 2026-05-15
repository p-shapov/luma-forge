## ADDED Requirements

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

## MODIFIED Requirements

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
