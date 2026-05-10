## ADDED Requirements

### Requirement: Native app state owns service wiring

The Native Layer SHALL own production dependency composition in managed native application state rather than in individual Tauri command handlers.

#### Scenario: Command invokes a native application service

- **WHEN** a Tauri command handler receives a command request
- **THEN** the handler SHALL map the request into native service input before invoking the application service
- **AND** the handler SHALL obtain production service dependencies from managed native application state
- **AND** the handler MUST NOT construct provider clients, secret stores, bundled catalog readers, or workspace repositories directly as part of command-specific business flow wiring

#### Scenario: Native app starts

- **WHEN** the Tauri app initializes native runtime state
- **THEN** the Native Layer SHALL register managed state that can provide production services for Provider Setup and Workspace Setup commands
- **AND** the managed state SHALL keep business workflow decisions inside application services rather than inside the state object

### Requirement: Workspace catalog runtime is shared outside handlers

The Native Layer SHALL manage Workspace Catalog path resolution, SQLite connection, and migration through native application state instead of repeating that runtime setup in workspace command handlers.

#### Scenario: Workspace command needs catalog access

- **WHEN** a Workspace Setup command needs to read or write the local Workspace Catalog
- **THEN** the command SHALL obtain catalog access through managed native application state
- **AND** the command handler MUST NOT resolve the app data directory or open and migrate the SQLite Workspace Catalog directly

#### Scenario: Workspace catalog initialization fails

- **WHEN** managed native application state cannot initialize or access the SQLite Workspace Catalog for a command
- **THEN** the command SHALL fail with the existing UI-safe Workspace Catalog or local storage error semantics
- **AND** the command MUST NOT return partial Workspace Catalog data as authoritative

#### Scenario: Multiple commands access the Workspace Catalog

- **WHEN** multiple native commands access the Workspace Catalog during one app runtime
- **THEN** the Native Layer SHALL reuse managed catalog access after successful initialization
- **AND** repeated command handling MUST NOT perform independent SQLite connection and migration setup for each command invocation

### Requirement: Operation coordinators are native runtime state

The Native Layer SHALL keep cross-command operation coordinators in managed native application state rather than constructing or owning them inside command modules.

#### Scenario: Provider setup operation is serialized

- **WHEN** Provider Setup, Provider Setup deletion, or Workspace creation needs provider setup serialization
- **THEN** the command SHALL acquire the provider setup operation guard through managed native application state
- **AND** serialization behavior SHALL remain consistent with the existing provider setup coordinator semantics

#### Scenario: Workspace operation coordination is introduced

- **WHEN** a native workflow needs per-Workspace operation serialization
- **THEN** the coordinator SHALL be owned by managed native application state
- **AND** commands participating in that workflow SHALL use the shared coordinator instead of defining command-local locks
