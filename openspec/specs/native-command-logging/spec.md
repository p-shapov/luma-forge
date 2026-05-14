# native-command-logging Specification

## Purpose

Define native-side command logging behavior for local debugging without exposing secrets, raw command payloads, provider transport details, or unsafe diagnostics.

## Requirements

### Requirement: Native command logs are written through the Tauri logging plugin

The Native Layer SHALL initialize an official Tauri logging plugin sink for native-side logs.

#### Scenario: Native app starts

- **WHEN** the Tauri application is built
- **THEN** the Native Layer SHALL initialize the official Tauri logging plugin
- **AND** native log records SHALL be routed to the configured plugin targets

#### Scenario: Frontend logging is not part of this change

- **WHEN** the Client runs in this change
- **THEN** the Client MUST NOT be required to initialize or call the Tauri logging plugin JavaScript API

### Requirement: Native command boundary emits safe lifecycle logs

The Native Layer SHALL log safe lifecycle records for Tauri command execution at the command boundary.

#### Scenario: Command execution starts

- **WHEN** a Tauri command handler begins executing a native command
- **THEN** the Native Layer SHALL log a command start record
- **AND** the record SHALL include the command name
- **AND** the record SHALL include an operation identifier for correlating records from the same command execution

#### Scenario: Command execution succeeds

- **WHEN** a Tauri command handler returns a successful command response
- **THEN** the Native Layer SHALL log a command success record
- **AND** the record SHALL include the command name, operation identifier, success outcome, and elapsed time

#### Scenario: Command execution fails with a native command error

- **WHEN** a Tauri command handler returns a structured native command error
- **THEN** the Native Layer SHALL log a command failure record
- **AND** the record SHALL include the command name, operation identifier, failure outcome, elapsed time, and UI-safe native command error metadata
- **AND** the UI-safe error metadata MAY include error code, retryability, field, reason, and recovery action

### Requirement: Native command logs exclude sensitive values

Native command logging MUST NOT persist secrets, raw credential-bearing payloads, or unsafe implementation diagnostics.

#### Scenario: Provider setup command receives an API key

- **WHEN** the Client submits a Provider API Key to `setup_gpu_cloud_provider`
- **THEN** native command logs MUST NOT include the submitted Provider API Key
- **AND** native command logs MUST NOT include the raw command request payload

#### Scenario: Native command uses a stored Provider API Key

- **WHEN** a native command reads or uses a stored Provider API Key
- **THEN** native command logs MUST NOT include the stored Provider API Key
- **AND** native command logs MUST NOT include raw keyring diagnostics

#### Scenario: Native command calls a provider API

- **WHEN** a native command causes the Native Layer to call a provider API
- **THEN** native command logs MUST NOT include bearer headers, raw provider request bodies, raw provider response bodies, or raw provider transport details
- **AND** native command logs MAY include UI-safe provider identifiers and UI-safe command error metadata

### Requirement: Application services remain independent from direct logging

Application services SHALL NOT be required to depend on Tauri runtime APIs or direct logging macros for native command logging.

#### Scenario: Command boundary logs a service result

- **WHEN** a Tauri command calls an application service and receives a result
- **THEN** the command boundary SHALL log the command outcome without requiring the service to perform direct logging

#### Scenario: Future service-level diagnostics are needed

- **WHEN** a future multi-phase workflow needs internal service diagnostics that command boundary logs cannot express
- **THEN** the application service SHALL receive diagnostics through a Tauri-independent dependency injection boundary
- **AND** the diagnostics boundary SHALL expose typed, safe diagnostic events rather than arbitrary raw log strings

### Requirement: Tracing is not used for native command logging

Native command logging SHALL use the Rust `log` facade and the Tauri logging plugin instead of the `tracing` stack for this change.

#### Scenario: Native command logging dependencies are configured

- **WHEN** native command logging is implemented
- **THEN** the native crate SHALL depend on the Rust `log` facade and the official Tauri logging plugin
- **AND** the native crate MUST NOT retain direct `tracing` or `tracing-subscriber` dependencies solely for this logging capability

#### Scenario: Existing native diagnostic log usage is preserved

- **WHEN** existing native diagnostic log statements are migrated
- **THEN** they SHALL continue to log only UI-safe diagnostic metadata
- **AND** they SHALL use the same native logging stack as command boundary logs
