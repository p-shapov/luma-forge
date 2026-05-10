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
- **AND** Workspace Setup command names and UI-safe error semantics SHALL remain compatible with the existing command contract
- **AND** Workspace Setup command payload shape changes SHALL be reflected in generated TypeScript bindings and the corresponding Workspace Setup specification delta
