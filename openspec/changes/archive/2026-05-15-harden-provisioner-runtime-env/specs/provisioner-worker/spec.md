## ADDED Requirements

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

## MODIFIED Requirements

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
