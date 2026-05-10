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

The Native Layer SHALL keep generated frontend binding derives on command-facing DTOs owned by the Tauri command boundary rather than on domain models. Domain models MAY derive native serialization traits needed for bundled catalog parsing, local persistence, or native snapshot serialization, but they MUST NOT derive `specta::Type` or other generated frontend binding traits solely to satisfy command payload generation.

#### Scenario: Command response exposes domain data

- **WHEN** a command returns data derived from domain models
- **THEN** the command response DTO SHALL derive the generated binding traits needed by Tauri/Specta
- **AND** the corresponding domain model MUST NOT be required to derive `specta::Type`
- **AND** the command boundary SHALL map the domain result into the generated command response DTO before returning it to React

#### Scenario: Command request enters application service

- **WHEN** a command receives a generated request DTO from React
- **THEN** the command or command-adjacent mapper SHALL map the DTO into domain values or a service input composed of domain values before business validation
- **AND** domain modules MUST NOT depend on Tauri command handlers
- **AND** application services MUST NOT depend on command-owned DTO modules

#### Scenario: Provider Setup command DTOs are generated

- **WHEN** Provider Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Provider Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Provider Setup domain models and services MUST NOT derive `specta::Type`
- **AND** Provider Setup command names, serialized payload fields, and UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Workspace Setup command DTOs are generated

- **WHEN** Workspace Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Workspace Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Workspace Setup domain models and services MUST NOT derive `specta::Type`
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

### Requirement: Provider-specific profile config is provider-discriminated domain data

Provider-specific profile configuration used by bundled catalogs, placement plans, and workspace snapshots SHALL be modeled as provider-discriminated domain data. Provider-specific HTTP, GraphQL, authentication, transport, and response DTOs SHALL remain owned by provider implementation modules.

#### Scenario: RunPod profile config is parsed from bundled catalogs

- **WHEN** bundled catalog data includes RunPod-specific provisioning or endpoint profile configuration
- **THEN** the RunPod-specific catalog/profile config structs SHALL be part of the domain-owned profile model
- **AND** the bundled catalog module SHALL parse and validate domain profile data directly
- **AND** the bundled catalog module MUST NOT require a parallel workspace application contract model solely for serialization

#### Scenario: Workspace setup validates selected profiles

- **WHEN** Workspace Setup validates selected Provisioning Profile and Endpoint Profile data
- **THEN** it SHALL validate provider-specific config through provider-discriminated domain profile variants
- **AND** it MUST NOT map selected profile data through a service-facing workspace contract layer before applying domain validation

### Requirement: Domain models remain independent from command and provider transport boundaries

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL response shapes, command handlers, Tauri runtime APIs, secure-storage implementations, and generated frontend binding requirements. Domain models MAY include provider-discriminated profile and placement variants when those variants represent LumaForge catalog, placement, or workspace state rather than provider transport payloads.

#### Scenario: Provider-specific profile data is needed

- **WHEN** profile and placement data include RunPod-specific catalog configuration
- **THEN** the provider-specific catalog configuration SHALL be represented by provider-discriminated domain profile or placement variants
- **AND** domain profile and placement types MUST NOT depend on provider HTTP or GraphQL response DTOs

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

#### Scenario: Domain model is used in command output

- **WHEN** a domain model must be returned to React
- **THEN** the command boundary SHALL expose a command DTO mapped from the domain model
- **AND** the domain model MUST NOT derive generated frontend binding traits solely to satisfy command output requirements

### Requirement: Workspace persistence stores provider identifiers from workspace data

Workspace catalog persistence SHALL serialize and deserialize domain Workspace records as the authoritative JSON payload, SHALL derive persisted provider identifiers from the workspace record being stored, and SHALL reject persisted Workspace rows whose indexed data is inconsistent with the serialized Workspace payload.

#### Scenario: Workspace is inserted

- **WHEN** the Workspace Catalog inserts a Workspace record
- **THEN** the stored `gpu_cloud_provider_id` column SHALL be derived from `workspace.gpu_cloud_provider_id`
- **AND** persistence MUST NOT hardcode the v1 provider identifier

#### Scenario: Workspace is re-read after insert

- **WHEN** the Workspace Catalog re-reads a persisted Workspace record
- **THEN** the returned Workspace SHALL be deserialized as a domain Workspace
- **AND** the returned Workspace SHALL match the serialized Workspace payload
- **AND** the indexed provider identifier SHALL remain consistent with that payload

#### Scenario: Workspace row data is inconsistent with payload

- **WHEN** the Workspace Catalog reads a persisted Workspace row whose indexed `id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, or `workflow_preset_id` value disagrees with the serialized Workspace payload
- **THEN** the Workspace Catalog SHALL reject the read as unavailable
- **AND** the inconsistent Workspace MUST NOT be returned as authoritative durable state

### Requirement: Domain modules do not use broad unused-code suppressions

Native domain modules MUST NOT use broad `#[allow(dead_code)]` or `#![allow(dead_code)]` workarounds. Domain types, functions, and modules SHALL either participate in live application behavior, represent spec-defined near-term domain vocabulary with a targeted explanatory `#[allow(dead_code)]`, or be removed until live behavior requires them.

#### Scenario: Domain code is introduced

- **WHEN** a native domain type, function, or module is added
- **THEN** it SHALL be used by live native application behavior or tests that exercise live behavior
- **AND** the implementation MUST NOT suppress unused-code warnings with broad `dead_code` allowances

#### Scenario: Domain code is no longer used

- **WHEN** a native domain type, function, or module is no longer used by live native application behavior
- **THEN** it SHALL be removed or reconnected to the behavior that owns its invariant
- **AND** it MUST NOT remain in the domain as speculative placeholder code behind a broad `dead_code` allowance

#### Scenario: Spec-defined lifecycle vocabulary is ahead of implementation

- **WHEN** a native domain enum variant is part of an accepted flow specification but the implementation that constructs it has not landed yet
- **THEN** the enum MAY use a targeted `#[allow(dead_code)]`
- **AND** the allowance MUST have an adjacent comment naming the upcoming flow or behavior that will construct the currently unused vocabulary

### Requirement: Workspace lifecycle is domain-authored

Workspace lifecycle state construction and transition rules SHALL be owned by domain code. Application services MAY orchestrate prerequisites, persistence, provider calls, and command mapping, but they MUST NOT hand-author lifecycle-bearing Workspace records when a domain constructor or transition exists for that behavior.

#### Scenario: Application service creates lifecycle-bearing Workspace state

- **WHEN** an application service needs to create or change Workspace lifecycle state
- **THEN** it SHALL call a domain Workspace constructor or transition method for that lifecycle behavior
- **AND** the domain Workspace model MUST NOT depend on application services, Tauri command handlers, command DTOs, SQLite repositories, provider clients, or generated frontend binding traits

#### Scenario: Workspace state crosses a native boundary

- **WHEN** domain-authored Workspace state is persisted or returned through a command
- **THEN** the Native Layer SHALL serialize the domain Workspace directly for native persistence or map it explicitly into the command DTO for generated frontend output
- **AND** generated command payload compatibility SHALL remain owned by the command boundary

### Requirement: Application services use domain-native contracts

Native application services SHALL accept domain values or service input structs composed of domain values, and SHALL return domain results instead of service-facing DTOs that duplicate command or domain models.

#### Scenario: Workspace Setup service receives a command request

- **WHEN** a Workspace Setup command receives a generated request DTO
- **THEN** the command boundary SHALL map the request into domain values before calling the Workspace Setup service
- **AND** the Workspace Setup service MUST NOT depend on `workspace_contracts.rs` or command DTO modules

#### Scenario: Provider Setup service returns setup state

- **WHEN** Provider Setup derives setup state from provider identity
- **THEN** the Provider Setup service SHALL return a domain `GpuCloudProviderSetup`
- **AND** the command boundary SHALL map that domain setup into the generated command response DTO

### Requirement: Domain validators own domain invariants

Native domain invariants SHALL be validated by domain-owned validators grouped by the concept or aggregate being validated. Infrastructure modules MAY parse, load, and adapt errors for their boundary, but they MUST NOT become the owner of reusable domain validation rules.

#### Scenario: Bundled catalog data is parsed

- **WHEN** bundled catalog readers deserialize workflow catalogs, provisioning profiles, endpoint profiles, or provider inventory into domain values
- **THEN** bundled parsing code SHALL keep parser and reader responsibilities in the `bundled` module
- **AND** bundled parsing code SHALL delegate domain invariant checks to concept-specific domain validators such as profile, placement, workflow/catalog, or provider inventory validators
- **AND** bundled parsing code SHALL translate domain validation failures into bundled-reader errors at the infrastructure boundary

#### Scenario: Workspace Setup validates placement

- **WHEN** Workspace Setup validates a submitted provider-discriminated Placement Plan
- **THEN** placement-specific invariants SHALL be checked through a domain-owned placement validator
- **AND** profile-specific invariants SHALL be checked through a domain-owned profile validator
- **AND** Workspace Setup services MUST NOT depend on bundled catalog validator modules for reusable domain rules

### Requirement: Module layout reflects native ownership boundaries

Native-layer modules SHALL be organized so that file and directory boundaries match ownership responsibilities.

#### Scenario: Workspace native code is organized

- **WHEN** workspace setup, workspace catalog, workspace contracts, and their tests are present
- **THEN** Workspace Setup code SHALL live under the `workspace_setup` module directory
- **AND** Workspace Catalog code SHALL live under the `workspace_catalog` module directory
- **AND** workspace test files SHALL be separate from implementation files

#### Scenario: Provider setup code is split

- **WHEN** provider setup code is split into multiple files
- **THEN** command contracts, application service orchestration, error mapping, and tests SHALL be separated by responsibility
- **AND** the split MUST NOT move provider-specific HTTP or GraphQL implementation details into provider setup

### Requirement: Workspace setup and catalog have separate native module boundaries

The Native Layer SHALL keep Workspace Setup orchestration code and Workspace Catalog persistence code in separate native module directories. Workspace Setup SHALL own setup service orchestration, setup service inputs, setup errors, and setup tests. Workspace Catalog SHALL own catalog repository traits, unavailable catalog adapters, SQLite catalog persistence, and catalog persistence tests.

#### Scenario: Workspace setup module owns setup orchestration

- **WHEN** native Workspace Setup service code is compiled
- **THEN** the service, setup input contracts, setup error type, and setup-focused tests SHALL be owned by a `workspace_setup` native module directory
- **AND** production Workspace Setup imports MUST NOT use obsolete flat `crate::workspace::workspace_setup_*` module paths

#### Scenario: Workspace catalog module owns persistence

- **WHEN** native Workspace Catalog persistence code is compiled
- **THEN** the repository trait, unavailable repository adapter, SQLite implementation, and catalog persistence tests SHALL be owned by a `workspace_catalog` native module directory
- **AND** production Workspace Catalog imports MUST NOT use obsolete flat `crate::workspace::workspace_catalog_*` module paths

#### Scenario: Public behavior remains compatible

- **WHEN** commands read bundled catalogs, fetch provider inventory, read the Workspace Catalog, or create a Draft Workspace after the module split
- **THEN** command names, generated frontend payloads, UI-safe error codes, Workspace Catalog SQLite schema, and persisted Workspace semantics SHALL remain compatible with the behavior before the split

### Requirement: Existing setup behavior is preserved

The boundary refactor SHALL preserve current GPU Cloud Provider Setup and Workspace Setup behavior.

#### Scenario: GPU Cloud Provider Setup command behavior is exercised

- **WHEN** existing GPU Cloud Provider Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible setup semantics

#### Scenario: Workspace Setup command behavior is exercised

- **WHEN** existing Workspace Setup tests run after the refactor
- **THEN** they SHALL continue to pass without changing the user-visible workspace setup semantics
