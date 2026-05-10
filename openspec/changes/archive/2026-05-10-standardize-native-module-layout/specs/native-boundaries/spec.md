## ADDED Requirements

### Requirement: Native module files use role-based names
Native Rust module files SHALL use names that describe the file's local responsibility within its parent module, and SHALL NOT repeat the parent module or provider name as a filename prefix when the parent directory already provides that context.

#### Scenario: Role file is declared inside an owning module
- **WHEN** a native module directory contains secondary responsibilities such as errors, contracts, repositories, readers, parsers, mappers, handlers, or tests
- **THEN** those files SHALL be named by role, such as `error.rs`, `contracts.rs`, `repository.rs`, `reader.rs`, `parser.rs`, `mapper.rs`, `handler.rs`, or `tests.rs`
- **AND** the corresponding `mod` declarations SHALL use the same role-based names

#### Scenario: Parent context would be repeated in a filename
- **WHEN** a native file is located under an owning module directory such as `provider_setup`, `workspace_setup`, `workspace_catalog`, `bundled_catalog`, `provider`, `provider/runpod`, or a command submodule
- **THEN** the file name MUST NOT include redundant prefixes such as `provider_setup_`, `workspace_setup_`, `workspace_catalog_`, `bundled_catalog_`, `provider_client_`, `runpod_`, or `workspace_command_`

### Requirement: Bundled catalog infrastructure has an explicit module root
Bundled catalog loading infrastructure SHALL live under a native module root named `bundled_catalog` instead of a generic `bundled` module root.

#### Scenario: Bundled catalog reader is imported
- **WHEN** native code imports the bundled catalog reader, parser, errors, or tests
- **THEN** those imports SHALL use the `bundled_catalog` module root
- **AND** production code MUST NOT import bundled catalog infrastructure through `crate::bundled`

#### Scenario: Future bundled assets are introduced
- **WHEN** a future native feature introduces bundled assets that are not Workflow Catalog, Provisioning Profile, or Endpoint Profile catalog infrastructure
- **THEN** those assets SHALL NOT be forced into the `bundled_catalog` module solely because they are bundled with the application

### Requirement: Primary native module code lives in the module root
Native Rust modules with a clear primary implementation SHALL keep that primary implementation in the module's `mod.rs`, while secondary responsibilities remain in role-named sibling files.

#### Scenario: Module has a central service or client
- **WHEN** a native module's primary responsibility is represented by a central service, coordinator-facing service API, provider client, or command handler set
- **THEN** that primary implementation SHALL live in the module's `mod.rs`
- **AND** sibling files SHALL be reserved for secondary roles such as errors, contracts, persistence adapters, mappers, and tests

#### Scenario: Module public surface is imported by other boundaries
- **WHEN** another native boundary imports the primary type or behavior of a module
- **THEN** the import SHALL be available from the owning module root
- **AND** imports of secondary details SHALL use the role-based child module path when those details are intentionally exposed

### Requirement: Native module layout refactors preserve behavior
Native module layout standardization SHALL preserve existing runtime behavior, generated command contracts, persisted state semantics, provider behavior, and UI-safe error semantics.

#### Scenario: Native module files are renamed
- **WHEN** native Rust files are renamed or primary implementations are moved into `mod.rs`
- **THEN** command names, generated TypeScript payloads, SQLite schema, domain validation behavior, provider request and response behavior, and UI-safe command error mappings SHALL remain compatible with the behavior before the refactor

#### Scenario: Native verification is run
- **WHEN** the module layout refactor is complete
- **THEN** `cargo test` SHALL pass
- **AND** `cargo clippy --fix --allow-dirty --allow-staged` SHALL complete successfully
- **AND** `cargo fmt` SHALL complete successfully
