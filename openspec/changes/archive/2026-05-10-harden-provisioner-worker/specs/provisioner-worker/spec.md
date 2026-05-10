## ADDED Requirements

### Requirement: Authorize worker API requests
The Provisioner Worker SHALL require bearer-token authorization for every HTTP endpoint when worker authorization is configured.

#### Scenario: Authorized request is accepted
- **WHEN** a worker bearer token is configured
- **AND** the client calls `GET /status`, `POST /start`, or `POST /cancel` with `Authorization: Bearer <configured-token>`
- **THEN** the Provisioner Worker SHALL process the request normally

#### Scenario: Unauthorized request is rejected
- **WHEN** a worker bearer token is configured
- **AND** the client omits the authorization header or provides a different token
- **THEN** the Provisioner Worker SHALL reject the request with `unauthorized`
- **AND** the Provisioner Worker MUST NOT start, cancel, or expose any provisioning job state mutation because of that request
- **AND** the response MUST NOT include the configured token

#### Scenario: Authorization is not configured
- **WHEN** no worker bearer token is configured
- **THEN** the Provisioner Worker SHALL continue to accept requests without an authorization header

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
The Provisioner Worker SHALL map expected failure classes to stable UI-safe error codes and messages.

#### Scenario: Git checkout fails
- **WHEN** a ComfyUI or Custom Node Git clone, fetch, or checkout operation fails
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `git_checkout_failed`
- **AND** the diagnostic message MUST NOT include secrets or raw credential-bearing command output

#### Scenario: Dependency installation fails
- **WHEN** ComfyUI or Custom Node dependency installation fails
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
- **WHEN** a request contains an unsafe workspace, Custom Node, requirements, or model asset install path
- **THEN** the Provisioner Worker SHALL reject the start request or mark the active job `failed` before the unsafe write or read
- **AND** the API response or job status SHALL include error code `path_validation_failed`

#### Scenario: Provisioning step times out
- **WHEN** a Git, dependency installation, or model download step exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the active operation where possible
- **AND** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL include error code `step_timeout`

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
The Provisioner Worker SHALL apply configured timeouts to external Git, dependency installation, and model download work.

#### Scenario: External step completes before timeout
- **WHEN** a Git command, dependency installation, or model download completes before its configured timeout
- **THEN** the Provisioner Worker SHALL continue provisioning normally

#### Scenario: External step exceeds timeout
- **WHEN** a Git command, dependency installation, or model download exceeds its configured timeout
- **THEN** the Provisioner Worker SHALL stop the operation where possible
- **AND** the Provisioner Worker SHALL fail the active job with `step_timeout`

## MODIFIED Requirements

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
