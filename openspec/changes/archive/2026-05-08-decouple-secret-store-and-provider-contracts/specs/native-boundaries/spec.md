## ADDED Requirements

### Requirement: Secret storage errors are use-case independent

Secret storage abstractions SHALL return secret-storage-owned errors instead of depending on Provider Setup, Workspace Setup, Provisioning, or Cleanup use-case error types.

#### Scenario: Provider Setup reads or writes secrets

- **WHEN** Provider Setup reads, replaces, or deletes a Provider API Key through the secret store
- **THEN** the secret store SHALL return secret-storage-owned failures
- **AND** Provider Setup SHALL map those failures into `ProviderSetupError`
- **AND** the secret store MUST NOT return `ProviderSetupError`

#### Scenario: Workspace Setup reads secrets

- **WHEN** Workspace Setup reads a Provider API Key through the secret store
- **THEN** the secret store SHALL return secret-storage-owned failures
- **AND** Workspace Setup SHALL map those failures into `WorkspaceSetupError`
- **AND** Workspace Setup MUST NOT convert from `ProviderSetupError` solely to handle secret store failures

#### Scenario: Stored Provider API Key is unreadable as a secret value

- **WHEN** the secure keyring contains a Provider API Key value that cannot be parsed as a valid Provider API Key
- **THEN** the secret store SHALL report a secret-storage-owned invalid stored key failure
- **AND** use-case mappings SHALL preserve the current UI-safe `invalid_provider_api_key` command behavior
- **AND** no command response, error, log, or diagnostic may include the stored secret value

### Requirement: Shared provider command DTOs are not owned by Provider Setup

Generated command DTOs that are shared by multiple native flows SHALL be owned by a neutral native contract module instead of a specific application use-case module.

#### Scenario: Provider id command DTO is used by multiple flows

- **WHEN** Provider Setup, Workspace Setup, workspace persistence, or tests need the command-facing `GpuCloudProviderId`
- **THEN** they SHALL import it from a neutral shared contract module
- **AND** they MUST NOT import it from the Provider Setup module unless they are Provider Setup internals

#### Scenario: Provider id command DTO maps to domain

- **WHEN** a command-facing `GpuCloudProviderId` enters native application logic
- **THEN** it SHALL continue to map explicitly to the domain `GpuCloudProviderId`
- **AND** the domain provider id MUST NOT derive generated binding traits solely to satisfy command DTO needs

#### Scenario: Generated frontend bindings are exported

- **WHEN** generated TypeScript command bindings are exported after moving the shared provider DTO
- **THEN** `GpuCloudProviderId` SHALL remain a UI-safe generated type with the same supported v1 value, `runpod`
- **AND** command request and response payload semantics SHALL remain compatible with existing Provider Setup and Workspace Setup behavior
