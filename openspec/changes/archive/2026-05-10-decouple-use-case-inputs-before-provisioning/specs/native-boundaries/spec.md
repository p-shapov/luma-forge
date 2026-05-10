## MODIFIED Requirements

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
