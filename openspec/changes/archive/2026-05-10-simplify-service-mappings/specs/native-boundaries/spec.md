## MODIFIED Requirements

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

## ADDED Requirements

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
