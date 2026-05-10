## ADDED Requirements

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

## MODIFIED Requirements

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
