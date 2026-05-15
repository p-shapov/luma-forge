## ADDED Requirements

### Requirement: Route worker API requests before body decoding

The Provisioner Worker SHALL determine whether an HTTP method and path target a supported endpoint before reading or decoding a request body.

#### Scenario: Unsupported POST endpoint has malformed body

- **WHEN** a client calls an unsupported `POST` endpoint with valid authorization and a malformed JSON body
- **THEN** the Provisioner Worker SHALL reject the request with the standard `not_found` worker error payload
- **AND** the Provisioner Worker MUST NOT return `invalid_json` for the unsupported endpoint
- **AND** the Provisioner Worker MUST NOT start, cancel, or otherwise mutate provisioning job state because of that request

#### Scenario: Unsupported POST endpoint has oversized body

- **WHEN** a client calls an unsupported `POST` endpoint with valid authorization and a `Content-Length` greater than the configured request-body limit
- **THEN** the Provisioner Worker SHALL reject the request with the standard `not_found` worker error payload
- **AND** the Provisioner Worker MUST NOT read the oversized body into memory
- **AND** the Provisioner Worker MUST NOT start, cancel, or otherwise mutate provisioning job state because of that request

#### Scenario: Supported POST endpoint still validates body

- **WHEN** a client calls `POST /start` or `POST /cancel` with valid authorization
- **THEN** the Provisioner Worker SHALL enforce the configured request body size before decoding request JSON
- **AND** the Provisioner Worker SHALL preserve existing malformed JSON, content-length, and payload validation error classifications for the supported endpoint

### Requirement: Return JSON worker errors for unsupported HTTP methods

The Provisioner Worker SHALL reject unsupported HTTP methods with the standard worker JSON error payload shape instead of returning stdlib HTML error responses.

#### Scenario: Unsupported method is unauthorized

- **WHEN** a client calls any worker path with an unsupported HTTP method and missing or invalid authorization
- **THEN** the Provisioner Worker SHALL reject the request with `unauthorized`
- **AND** the response body SHALL use the standard worker error payload shape with `code`, `reason_code`, and `message`
- **AND** the response MUST NOT include the configured bearer token or supplied authorization value

#### Scenario: Unsupported method is authorized

- **WHEN** a client calls any worker path with an unsupported HTTP method and valid authorization
- **THEN** the Provisioner Worker SHALL reject the request with a standard worker error payload
- **AND** the Provisioner Worker MUST NOT return an HTML error document
- **AND** the Provisioner Worker MUST NOT start, cancel, or otherwise mutate provisioning job state because of that request

### Requirement: Sanitize unexpected preparation failures

The Provisioner Worker SHALL handle unexpected preparation exceptions by recording a sanitized terminal job failure without exposing raw exception details through status responses or default thread traceback output.

#### Scenario: Unexpected preparation exception occurs

- **WHEN** preparation raises an exception that is not a known worker error and not cancellation
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `unexpected_error`
- **AND** `GET /status` SHALL include reason code `unexpected_exception`
- **AND** the diagnostic message and error payload MUST NOT include the original exception message, stack trace, request body, raw command output, environment dump, bearer token, provider API key, or credential-bearing URL
- **AND** the worker thread MUST NOT re-raise the original exception after recording the sanitized terminal failure

### Requirement: Keep worker status diagnostics safe for user-defined presets

The Provisioner Worker SHALL treat Workflow Preset names, Custom Node names, and unsafe preset-provided identifiers as untrusted input when producing status diagnostics. Model asset display names MAY be included during model asset download progress after display-name validation.

#### Scenario: Progress is reported for preset-declared Custom Node

- **WHEN** preparation reports progress while installing a preset-declared Custom Node
- **THEN** `GET /status` SHALL report the appropriate phase
- **AND** the diagnostic message MAY include the Custom Node identifier only after the identifier has passed worker schema validation
- **AND** the diagnostic message MUST NOT include the Custom Node display name or any unsafe identifier from the request payload

#### Scenario: Progress is reported for preset-declared model asset

- **WHEN** preparation reports progress while downloading a preset-declared model asset
- **THEN** `GET /status` SHALL report the appropriate phase
- **AND** the diagnostic message MAY include the model asset display name after the display name has passed worker schema validation
- **AND** the diagnostic message MAY include the model asset identifier only after the identifier has passed worker schema validation
- **AND** the diagnostic message MUST NOT include any unsafe identifier from the request payload

#### Scenario: Validation fails for missing preset-declared item

- **WHEN** final validation finds a missing preset-declared Custom Node or model asset
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** the terminal diagnostic message and error payload message MAY include the missing item's identifier only after the identifier has passed worker schema validation
- **AND** the terminal diagnostic message and error payload message MUST NOT include the item's display name or any unsafe identifier from the request payload

### Requirement: Validate preset-provided identifiers used by worker diagnostics

The Provisioner Worker SHALL validate preset-provided identifiers before accepting a start request when those identifiers may be stored in worker status, runtime metadata, install report names, or future structured error context.

#### Scenario: Preset identifier is safe

- **WHEN** a selected Workflow Preset contains identifiers made only from ASCII letters, ASCII digits, `.`, `_`, or `-`, with a length from 1 through 128 characters
- **THEN** the Provisioner Worker MAY accept those identifiers for preparation

#### Scenario: Preset identifier contains unsafe characters

- **WHEN** a selected Workflow Preset contains an identifier with whitespace, path separators, control characters, shell metacharacters, markup-significant characters, or non-ASCII characters
- **THEN** the Provisioner Worker SHALL reject the start request with `invalid_request`
- **AND** the response MUST NOT echo the raw unsafe identifier value
- **AND** the Provisioner Worker MUST NOT write to the mounted workspace volume

#### Scenario: Preset identifier is too long

- **WHEN** a selected Workflow Preset contains an identifier longer than 128 characters
- **THEN** the Provisioner Worker SHALL reject the start request with `invalid_request`
- **AND** the response MUST NOT echo the raw unsafe identifier value
- **AND** the Provisioner Worker MUST NOT write to the mounted workspace volume
