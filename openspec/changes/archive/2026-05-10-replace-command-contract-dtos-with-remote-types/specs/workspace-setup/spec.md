## MODIFIED Requirements

### Requirement: Workspace Setup uses domain-native catalog data

Workspace Setup SHALL parse, validate, provide, persist, and expose domain-native catalog and workspace models without a separate workspace application contract layer or duplicated command model graph.

#### Scenario: Bundled catalogs are read

- **WHEN** Workspace Setup reads bundled Workflow Catalog, Provisioning Profiles, or Endpoint Profiles
- **THEN** the bundled catalog reader SHALL return domain-native catalog and profile data
- **AND** the catalog reader MUST NOT return `workspace_contracts.rs` DTOs
- **AND** the command boundary SHALL expose generated TypeScript bindings for returned domain-native catalog and profile data without requiring duplicated runtime command DTO graphs

#### Scenario: Workspace catalog is read

- **WHEN** Workspace Setup reads the local Workspace Catalog
- **THEN** the workspace repository SHALL return domain-native Workspace Catalog data
- **AND** the command boundary SHALL expose the domain-native Workspace Catalog through a generated command response wrapper
- **AND** the command boundary MAY use command-owned remote generated binding metadata for nested Workspace Catalog domain types
- **AND** returned Workspace records SHALL expose provider-discriminated domain Placement Plan data in the generated command payload
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation

#### Scenario: Workspace creation receives a placement plan

- **WHEN** the Client submits a Workspace creation request
- **THEN** the generated command request SHALL require provider-discriminated domain Placement Plan data
- **AND** the submitted Placement Plan SHALL include the nested `gpu_cloud_provider_id` discriminator required by the domain Placement Plan shape
- **AND** the command boundary SHALL pass the submitted domain Placement Plan into the Workspace Setup service input without a parallel command Placement Plan DTO
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation
