## ADDED Requirements

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
- **THEN** the Native Layer SHALL map it explicitly into the appropriate application, persistence, or command contract shape
- **AND** generated command payload compatibility SHALL remain owned by the command boundary
