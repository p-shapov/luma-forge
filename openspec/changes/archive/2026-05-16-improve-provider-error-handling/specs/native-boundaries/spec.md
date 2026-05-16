## ADDED Requirements

### Requirement: Provider-local errors expose stable LumaForge-owned failure variants

Provider client implementations SHALL classify provider failures into stable LumaForge-owned provider errors and SHALL keep provider-specific response interpretation inside the provider implementation boundary.

#### Scenario: RunPod REST response is classified

- **WHEN** the RunPod provider implementation receives a REST response for a provisioning resource operation
- **THEN** it SHALL classify the response into a provider-local error when the response is not successful
- **AND** `401` and `403` SHALL map to authorization failure
- **AND** `404` SHALL map to provider resource not found
- **AND** `429` SHALL map to provider rate limiting
- **AND** `409` SHALL map to provider operation conflict
- **AND** `408` and `504` SHALL map to provider operation indeterminate
- **AND** other `4xx` statuses SHALL map to provider request rejection
- **AND** other non-success statuses SHALL map to provider API unavailability
- **AND** downstream modules MUST NOT inspect RunPod-specific error codes, message strings, or response envelopes
- **AND** the provider-local failure MUST NOT include Provider API Keys, bearer headers, raw request bodies, or raw response bodies

#### Scenario: RunPod GraphQL error is classified

- **WHEN** the RunPod provider implementation receives GraphQL errors from identity or inventory requests
- **THEN** obvious authentication-related errors SHALL map to authorization failure
- **AND** other GraphQL errors SHALL map to provider request rejection
- **AND** domain and command modules MUST NOT depend on RunPod GraphQL error message strings

#### Scenario: RunPod inventory HTTP status is classified

- **WHEN** the RunPod provider implementation receives a non-success HTTP status while fetching Provider Inventory
- **THEN** authorization statuses SHALL map to authorization failure
- **AND** rate limiting SHALL map to provider rate limiting
- **AND** non-authentication `4xx` statuses SHALL map to provider request rejection
- **AND** other non-success statuses SHALL map to provider API unavailability

### Requirement: Provider registry maps provider errors to use-case errors

The provider registry SHALL map provider-local errors into use-case errors for Provider Setup, Workspace Setup, and Workspace Provisioning without leaking provider transport details.

#### Scenario: Provider setup and workspace setup map provider errors

- **WHEN** Provider Setup or Workspace Setup receives a provider-local failure through the provider registry
- **THEN** the registry SHALL map the provider-local error into the corresponding use-case error
- **AND** unauthorized provider keys SHALL remain non-retryable setup recovery failures
- **AND** provider API unavailability and rate limiting SHALL remain retryable provider availability failures
- **AND** provider request rejection SHALL remain distinct from provider API unavailability

#### Scenario: Workspace Provisioning maps provider errors

- **WHEN** Workspace Provisioning receives a provider-local failure through the provider registry
- **THEN** the registry SHALL map rate limiting to a provider rate-limited provisioning error
- **AND** the registry SHALL map request rejection to a provider request-rejected provisioning error
- **AND** the mapped error MUST NOT expose provider transport details, Provider API Keys, bearer headers, raw provider payloads, or provider-specific error codes as domain contracts

### Requirement: Command errors expose stable UI-safe provider recovery metadata

The Tauri command boundary SHALL map provider-related use-case errors into stable UI-safe command error metadata that reflects provider recovery semantics.

#### Scenario: Provider request is rejected

- **WHEN** a native command fails because the Provider rejected a UI-controlled request value or placement selection
- **THEN** the command error SHALL use the stable LumaForge-owned `provider_request_rejected` code or reason
- **AND** the command error SHALL mark retrying the same request as not retryable
- **AND** the recovery action SHALL guide the Client to change or reselect the invalid request value when applicable
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider is rate limited

- **WHEN** a native command fails because the Provider reports rate limiting
- **THEN** the command error SHALL use the stable LumaForge-owned `provider_rate_limited` code or reason
- **AND** the command error SHALL mark the failure as retryable when repeating the same command is safe
- **AND** the command error SHALL expose only UI-safe code, message, retryability, reason, field, and recovery action metadata

#### Scenario: Provider is unavailable

- **WHEN** a native command fails because the Provider is unavailable, timed out, or temporarily unable to complete a safe operation
- **THEN** the command error SHALL mark the failure as retryable when repeating the same command is safe
- **AND** the command error SHALL expose only UI-safe code, message, retryability, reason, field, and recovery action metadata
