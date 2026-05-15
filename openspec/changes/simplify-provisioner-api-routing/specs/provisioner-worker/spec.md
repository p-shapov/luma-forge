## ADDED Requirements

### Requirement: Preserve worker API behavior during HTTP adapter refactors

The Provisioner Worker SHALL preserve its public HTTP API behavior when its internal request-handler routing structure is refactored.

#### Scenario: Existing worker endpoints remain available

- **WHEN** a client calls `GET /status`, `POST /start`, or `POST /cancel` with valid authorization and a valid request where a body is required
- **THEN** the Provisioner Worker SHALL process the request according to the existing endpoint contract
- **AND** the Provisioner Worker SHALL return the same status snapshot and error payload shapes defined for those endpoints

#### Scenario: Unsupported endpoint is rejected

- **WHEN** a client calls an endpoint other than `GET /status`, `POST /start`, or `POST /cancel`
- **THEN** the Provisioner Worker SHALL reject the request with the existing not-found worker error payload
- **AND** the Provisioner Worker MUST NOT start, cancel, or otherwise mutate provisioning job state because of that request

#### Scenario: Shared HTTP safeguards are preserved

- **WHEN** a request is unauthorized, has malformed `Content-Length`, exceeds the configured request-body limit, contains invalid JSON, or fails payload validation
- **THEN** the Provisioner Worker SHALL reject the request with the existing worker error classification for that condition
- **AND** the response MUST NOT include bearer tokens, provider API keys, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

### Requirement: Keep provisioner worker API dependency-light

The Provisioner Worker SHALL keep its local HTTP API implementation dependency-light unless the worker API grows beyond the current small endpoint set or requires framework-level capabilities.

#### Scenario: Routing readability is improved for the current endpoint set

- **WHEN** the worker API contains only `GET /status`, `POST /start`, and `POST /cancel`
- **THEN** the Provisioner Worker SHALL implement routing without adding a new web framework runtime dependency
- **AND** the implementation SHALL keep endpoint-specific work separated from shared authorization, request parsing, and response serialization concerns
