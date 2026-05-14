## MODIFIED Requirements

### Requirement: Command DTOs own generated binding concerns

The Native Layer SHALL keep generated frontend binding concerns owned by the Tauri command boundary rather than by domain models. Command-facing DTOs MAY derive generated binding traits directly, and command modules MAY provide generated binding metadata for domain models through command-owned remote type exports. Domain models MAY derive native serialization traits needed for bundled catalog parsing, local persistence, or native snapshot serialization, but they MUST NOT derive `specta::Type` or other generated frontend binding traits solely to satisfy command payload generation.

#### Scenario: Command response exposes domain data through a command wrapper

- **WHEN** a command returns data derived from domain models
- **THEN** the command response shape SHALL remain owned by the command boundary
- **AND** the command boundary MAY expose nested domain model data through command-owned remote generated binding metadata
- **AND** the corresponding domain model MUST NOT be required to derive `specta::Type`
- **AND** generated command payload field and discriminant changes SHALL be explicit in the OpenSpec change when the command contract intentionally migrates
- **AND** UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Command request enters application service

- **WHEN** a command receives a generated request DTO from React
- **THEN** the command or command-adjacent mapper SHALL convert command-specific wrapper data into domain values or a service input composed of domain values before business validation
- **AND** command request DTOs MAY contain nested domain values when the command boundary owns generated binding metadata for those domain types
- **AND** domain modules MUST NOT depend on Tauri command handlers
- **AND** application services MUST NOT depend on command-owned DTO modules

#### Scenario: Provider Setup command DTOs are generated

- **WHEN** Provider Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Provider Setup request and response DTOs SHALL be owned by the command boundary
- **AND** Provider Setup domain models and services MUST NOT derive `specta::Type`
- **AND** Provider Setup command names, serialized payload fields, and UI-safe error semantics SHALL remain compatible with the existing command contract

#### Scenario: Workspace Setup command DTOs are generated

- **WHEN** Workspace Setup commands are exported as generated TypeScript bindings
- **THEN** the generated Workspace Setup request and response wrappers SHALL be owned by the command boundary
- **AND** Workspace Setup command modules MAY provide command-owned remote generated binding metadata for Workspace Setup domain models
- **AND** Workspace Setup domain models and services MUST NOT derive `specta::Type`
- **AND** Workspace Setup command payload shape changes SHALL be reflected in generated TypeScript bindings and the corresponding Workspace Setup specification delta
- **AND** generated Workspace Setup bindings MUST NOT expose Provisioning Profile or Endpoint Profile command types after profiles are removed

### Requirement: Domain models remain independent from command and provider transport boundaries

Domain models SHALL remain independent from provider-specific HTTP shapes, GraphQL response shapes, command handlers, Tauri runtime APIs, secure-storage implementations, runtime environment variable readers, and generated frontend binding requirements. Domain models MAY include provider-discriminated placement variants when those variants represent LumaForge workspace state rather than provider transport payloads.

#### Scenario: Provider-specific placement data is needed

- **WHEN** placement data includes RunPod-specific workspace placement selections
- **THEN** those selections MAY be represented by provider-discriminated domain placement variants
- **AND** domain placement types MUST NOT depend on provider HTTP or GraphQL response DTOs
- **AND** domain placement types MUST NOT contain Provisioning Profile or Endpoint Profile snapshots

#### Scenario: Provider API response is parsed

- **WHEN** a provider module parses a provider API response
- **THEN** provider response DTOs and mapping code SHALL remain inside the provider implementation boundary
- **AND** domain modules MUST NOT import provider response DTOs

#### Scenario: Domain model is used in command output

- **WHEN** a domain model must be returned to React
- **THEN** the command boundary SHALL expose a command DTO mapped from the domain model
- **AND** the domain model MUST NOT derive generated frontend binding traits solely to satisfy command output requirements

## REMOVED Requirements

### Requirement: Provider-specific profile config is provider-discriminated domain data

**Reason**: Provisioning Profiles and Endpoint Profiles are removed as domain concepts.

**Migration**: Provider-specific runtime values that remain variable move to Native build-time configuration. Fixed RunPod values are removed from profile contracts and deferred until provider provisioning code consumes them.
