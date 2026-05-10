# native-boundaries Specification

## Purpose
TBD - created by archiving change refactor-native-boundaries. Update Purpose after archive.
## Requirements
### Requirement: Command boundary owns generated command errors

The Native Layer SHALL keep command-safe error response DTOs owned by the Tauri command boundary, not by a specific application use case.

#### Scenario: Use-case error is returned from a command

- **WHEN** a native application service returns a use-case error
- **THEN** the Tauri command handler SHALL map that error into a UI-safe command error response
- **AND** the application service MUST NOT own the shared generated command error DTO

#### Scenario: Command error is exposed to React

- **WHEN** generated command bindings expose an error shape to React
- **THEN** the exposed error SHALL contain only a UI-safe code, UI-safe message, and retryability flag
- **AND** the exposed error MUST NOT include provider secrets or provider transport details

### Requirement: Provider setup recovery-required errors are explicit

The Native command boundary SHALL expose a UI-safe provider setup recovery-required error when a failed setup attempt may have left partial local setup state that could not be rolled back.

#### Scenario: Provider setup rollback fails

- **WHEN** Provider Setup reports that setup finalization failed after writing a Provider API Key and rollback deletion also failed
- **THEN** the Tauri command handler SHALL map the failure to `provider_setup_recovery_required`
- **AND** the generated command error SHALL include only a UI-safe code, UI-safe message, and retryability flag
- **AND** the generated command error MUST NOT include the submitted Provider API Key, stored Provider API Key, provider transport details, or keyring diagnostics
- **AND** the generated command error SHALL mark retrying the same setup command as not retryable

### Requirement: Provider clients are use-case independent

Provider client implementations SHALL return provider-local results and errors instead of depending on setup, workspace setup, provisioning, or cleanup use-case error types.

#### Scenario: RunPod identity validation fails

- **WHEN** the RunPod client observes an identity transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `ProviderSetupError`

#### Scenario: RunPod inventory lookup fails

- **WHEN** the RunPod client observes an inventory transport, authorization, or response parsing failure
- **THEN** the RunPod client SHALL return a provider-local error
- **AND** the RunPod client MUST NOT return `WorkspaceSetupError`

### Requirement: Provider registry maps provider errors to use-case errors

The provider registry SHALL adapt provider-local client errors into the use-case error type required by each gateway trait implementation.

#### Scenario: Provider setup validates identity

- **WHEN** Provider Setup asks the provider registry to validate identity
- **THEN** the provider registry SHALL call the provider client
- **AND** the provider registry SHALL map provider-local failures into `ProviderSetupError`

#### Scenario: Workspace Setup reads inventory

- **WHEN** Workspace Setup asks the provider registry to fetch provider inventory
- **THEN** the provider registry SHALL call the provider client
- **AND** the provider registry SHALL map provider-local failures into `WorkspaceSetupError`

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

### Requirement: Command DTOs own generated binding concerns

The Native Layer SHALL keep generated frontend binding derives on command-facing DTOs owned by the Tauri command boundary rather than on pure domain models or application/service contracts. Application/service contracts MAY keep serialization derives needed for native parsing, persistence, or snapshot serialization, but they MUST NOT be required to derive `specta::Type` solely to satisfy generated frontend bindings.

#### Scenario: Command response exposes application data

- **WHEN** a command returns data derived from domain models or application/service contracts
- **THEN** the command response DTO SHALL derive the generated binding traits needed by Tauri/Specta
- **AND** the corresponding domain model or application/service contract MUST NOT be required to derive `specta::Type`
- **AND** the command boundary SHALL map the application/service result into the generated command response DTO before returning it to React

#### Scenario: Command request enters application service

- **WHEN** a command receives a generated request DTO from React
- **THEN** the command or command-adjacent mapper SHALL map the DTO into application/service contract types before business validation
- **AND** domain modules MUST NOT depend on Tauri command handlers
- **AND** application services MUST NOT depend on command-owned DTO modules

#### Scenario: Provider Setup command DTOs are generated

- **WHEN** Provider Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Provider Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Provider Setup application/service contracts MUST NOT derive `specta::Type`
- **AND** Provider Setup command names, serialized payload fields, and UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Workspace Setup command DTOs are generated

- **WHEN** Workspace Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Workspace Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Workspace Setup application/service contracts MUST NOT derive `specta::Type`
- **AND** Workspace Setup command names, serialized payload fields, and UI-safe error semantics SHALL remain compatible with the existing command contract

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

### Requirement: Provider-specific profile config is provider-owned

Provider-specific profile configuration contracts SHALL be owned by the provider boundary that understands those fields.

#### Scenario: RunPod profile config is parsed from bundled catalogs

- **WHEN** bundled catalog data includes RunPod-specific provisioning or endpoint profile configuration
- **THEN** the RunPod-specific config structs SHALL live under the RunPod provider boundary
- **AND** the bundled catalog module MUST NOT be the owner of shared workspace profile contract types

#### Scenario: Workspace setup validates selected profiles

- **WHEN** Workspace Setup validates selected Provisioning Profile and Endpoint Profile data
- **THEN** it MAY compare provider-specific config payloads through provider-owned RunPod contract types
- **AND** it MUST NOT import RunPod-specific config types from `domain`

### Requirement: Domain models remain provider-agnostic

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL shapes, provider template identifiers, command handlers, Tauri runtime APIs, secure-storage implementations, serialization requirements, and generated frontend binding requirements.

#### Scenario: Provider-specific profile data is needed

- **WHEN** profile contracts include RunPod-specific configuration
- **THEN** the provider-specific configuration SHALL live in provider boundary contracts
- **AND** generic domain profile and placement types MUST NOT depend on RunPod-specific config types

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

#### Scenario: Domain model is used in command output

- **WHEN** a domain model must be returned to React
- **THEN** the command boundary SHALL expose a command DTO mapped from the domain model
- **AND** the domain model MUST NOT derive generated frontend binding traits solely to satisfy command output requirements

### Requirement: Workspace persistence stores provider identifiers from workspace data

Workspace catalog persistence SHALL derive persisted provider identifiers from the workspace record being stored, and SHALL reject persisted Workspace rows whose indexed data is inconsistent with the serialized Workspace payload.

#### Scenario: Workspace is inserted

- **WHEN** the Workspace Catalog inserts a Workspace record
- **THEN** the stored `gpu_cloud_provider_id` column SHALL be derived from `workspace.gpu_cloud_provider_id`
- **AND** persistence MUST NOT hardcode the v1 provider identifier

#### Scenario: Workspace is re-read after insert

- **WHEN** the Workspace Catalog re-reads a persisted Workspace record
- **THEN** the returned Workspace SHALL match the serialized Workspace payload
- **AND** the indexed provider identifier SHALL remain consistent with that payload

#### Scenario: Workspace row data is inconsistent with payload

- **WHEN** the Workspace Catalog reads a persisted Workspace row whose indexed `id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, or `workflow_preset_id` value disagrees with the serialized Workspace payload
- **THEN** the Workspace Catalog SHALL reject the read as unavailable
- **AND** the inconsistent Workspace MUST NOT be returned as authoritative durable state

### Requirement: Module layout reflects native ownership boundaries

Native-layer modules SHALL be organized so that file and directory boundaries match ownership responsibilities.

#### Scenario: Workspace native code is organized

- **WHEN** workspace setup, workspace catalog, workspace contracts, and their tests are present
- **THEN** they SHALL live under the workspace module directory
- **AND** workspace test files SHALL be separate from implementation files

#### Scenario: Provider setup code is split

- **WHEN** provider setup code is split into multiple files
- **THEN** command contracts, application service orchestration, error mapping, and tests SHALL be separated by responsibility
- **AND** the split MUST NOT move provider-specific HTTP or GraphQL implementation details into provider setup

### Requirement: Existing setup behavior is preserved

The boundary refactor SHALL preserve current GPU Cloud Provider Setup and Workspace Setup behavior.

#### Scenario: GPU Cloud Provider Setup command behavior is exercised

- **WHEN** existing GPU Cloud Provider Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible setup semantics

#### Scenario: Workspace Setup command behavior is exercised

- **WHEN** existing Workspace Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible workspace setup semantics
