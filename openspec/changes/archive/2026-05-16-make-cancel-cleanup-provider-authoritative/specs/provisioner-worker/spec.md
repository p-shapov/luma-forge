## MODIFIED Requirements

### Requirement: Expose Provisioner Worker HTTP API

The Provisioner Worker SHALL expose an HTTP API with `POST /start` and `GET /status` from inside the provisioner container.

#### Scenario: Worker starts idle

- **WHEN** the provisioner container starts
- **THEN** the Provisioner Worker SHALL start an HTTP server
- **AND** the Provisioner Worker SHALL report `idle` status before any `/start` request is accepted
- **AND** the Provisioner Worker MUST NOT prepare the ComfyUI environment before `/start`

#### Scenario: Worker API includes required endpoints

- **WHEN** a client calls the worker API
- **THEN** `POST /start` and `GET /status` SHALL be available
- **AND** `POST /cancel` MUST NOT be available
- **AND** the worker API MUST NOT expose Provider API Keys or Hugging Face API keys in any response

### Requirement: Report provisioning status

The Provisioner Worker SHALL report UI-safe provisioning job status through `GET /status`.

#### Scenario: Worker is idle

- **WHEN** no provisioning job has been started
- **THEN** `GET /status` SHALL return status `idle`
- **AND** the response SHALL include no active job identifier
- **AND** the response MAY report no active phase
- **AND** the response MUST NOT include secrets, request bodies, raw command output, stack traces, or environment dumps

#### Scenario: Job is running

- **WHEN** a provisioning job is active
- **THEN** `GET /status` SHALL return the active job identifier, status `running`, current phase, updated timestamp, and optional progress percentage
- **AND** the current phase SHALL use a stable worker phase value for the active preparation step
- **AND** the response MAY include a UI-safe diagnostic message

#### Scenario: Job succeeds

- **WHEN** ComfyUI, Custom Nodes, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL mark the job `succeeded`
- **AND** `GET /status` SHALL report terminal success
- **AND** the terminal success response MAY report no active phase

#### Scenario: Job fails

- **WHEN** a provisioning step cannot complete safely
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL report terminal failure with UI-safe error metadata
- **AND** the terminal error metadata SHALL use the standard worker error payload shape with `code`, `reason_code`, and `message`
- **AND** the response MAY include a UI-safe diagnostic message
- **AND** the response MUST NOT include provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

### Requirement: Authorize worker API requests

The Provisioner Worker SHALL require bearer-token authorization for every HTTP endpoint.

#### Scenario: Authorized request is accepted

- **WHEN** the client calls `GET /status` or `POST /start` with `Authorization: Bearer <configured-token>`
- **THEN** the Provisioner Worker SHALL process the request normally

#### Scenario: Unauthorized request is rejected

- **WHEN** the client omits the authorization header or provides a different token
- **THEN** the Provisioner Worker SHALL reject the request with `unauthorized`
- **AND** the Provisioner Worker MUST NOT start or expose any provisioning job state mutation because of that request
- **AND** the response MUST NOT include the configured token

### Requirement: Bound worker request bodies

The Provisioner Worker SHALL enforce a configured maximum request body size before decoding request JSON.

#### Scenario: Request body is within the limit

- **WHEN** `POST /start` includes a valid `Content-Length` that is less than or equal to the configured maximum
- **THEN** the Provisioner Worker MAY read and parse the request body

#### Scenario: Request body is too large

- **WHEN** `POST /start` includes a `Content-Length` greater than the configured maximum
- **THEN** the Provisioner Worker SHALL reject the request with `request_too_large`
- **AND** the Provisioner Worker MUST NOT read the oversized body into memory
- **AND** the Provisioner Worker MUST NOT mutate provisioning job state

#### Scenario: Request body length is malformed

- **WHEN** `POST /start` includes a missing, negative, or non-integer `Content-Length`
- **THEN** the Provisioner Worker SHALL reject the request with `invalid_request`
- **AND** the Provisioner Worker MUST NOT mutate provisioning job state

### Requirement: Preserve worker API behavior during HTTP adapter refactors

The Provisioner Worker SHALL preserve its public HTTP API behavior when its internal request-handler routing structure is refactored.

#### Scenario: Existing worker endpoints remain available

- **WHEN** a client calls `GET /status` or `POST /start` with valid authorization and a valid request where a body is required
- **THEN** the Provisioner Worker SHALL process the request according to the existing endpoint contract
- **AND** the Provisioner Worker SHALL return the same status snapshot and error payload shapes defined for those endpoints

#### Scenario: Unsupported endpoint is rejected

- **WHEN** a client calls an endpoint other than `GET /status` or `POST /start`
- **THEN** the Provisioner Worker SHALL reject the request with the existing not-found worker error payload
- **AND** the Provisioner Worker MUST NOT start or otherwise mutate provisioning job state because of that request

## REMOVED Requirements

### Requirement: Cancel active provisioning

**Reason**: Destructive Workspace Provisioning cancellation is now owned by Native provider cleanup. Native terminates the RunPod provisioning pod instead of asking the in-pod worker process to cancel itself.

**Migration**: Clients SHALL cancel Workspace Provisioning through the Native Layer. Direct worker clients MUST treat `POST /cancel` as unsupported and use provider pod termination for destructive cancellation.
