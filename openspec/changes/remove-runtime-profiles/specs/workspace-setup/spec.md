## MODIFIED Requirements

### Requirement: Migrate Workspace Catalog persistence before use

The Native Layer SHALL apply required Workspace Catalog SQLite schema migrations before using the Workspace Catalog for reads, duplicate checks, inserts, or post-insert re-reads. Pre-production legacy Workspace JSON compatibility migrations are not required by the current app contract.

#### Scenario: Unversioned Workspace Catalog is migrated

- **WHEN** the Native Layer opens an existing Workspace Catalog that has no recorded persistence version
- **THEN** the Native Layer SHALL treat the catalog as version `0`
- **AND** the Native Layer SHALL apply every required schema migration up to the current persistence version before returning or writing Workspace records
- **AND** the Native Layer SHALL record the current persistence version only after all required migrations complete successfully

#### Scenario: Current Workspace Catalog is already migrated

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version matches the current application persistence version
- **THEN** the Native Layer SHALL use the existing Workspace records without rewriting them for legacy profile compatibility
- **AND** normal Workspace Catalog read, duplicate check, insert, and row consistency validation rules SHALL still apply

#### Scenario: Workspace Catalog was written by a newer app version

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version is greater than the current application persistence version
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_migration_failed`
- **AND** the Native Layer MUST NOT read, write, migrate, downgrade, or mutate Workspace records from the newer catalog version

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record after required Workspace Catalog persistence migrations have completed.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled Workflow Preset compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL apply required Workspace Catalog persistence migrations before checking duplicates or writing the new Workspace record
- **AND** the Native Layer SHALL construct the Draft Workspace through the domain Workspace model
- **AND** the domain-authored Workspace SHALL have lifecycle state `draft`
- **AND** the domain-authored Workspace SHALL have empty Persistent Storage Volume, active Provisioning Pod, Serverless Endpoint, and last Provisioning Pod snapshots
- **AND** the Native Layer SHALL persist the domain-authored Workspace as the authoritative Workspace Catalog record
- **AND** the Native Layer SHALL persist one Workspace Catalog entry in SQLite with lifecycle state `draft`
- **AND** the Native Layer SHALL re-read the persisted Workspace record from SQLite
- **AND** the Native Layer SHALL verify that the re-read Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL return the re-read Workspace record as authoritative
- **AND** Workspace creation MUST NOT require a live Provider identity check

#### Scenario: Duplicate Workspace UUID

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL apply required Workspace Catalog persistence migrations before evaluating the duplicate Workspace UUID
- **AND** the Native Layer SHALL reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record
- **AND** the Native Layer MUST NOT create a second Workspace record for the same Workspace UUID

#### Scenario: Provider API Key is missing during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Provider API Key is unreadable during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key cannot be parsed as a secret value
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT call the Provider to validate identity
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace Catalog write fails

- **WHEN** the Client submits a valid Workspace creation request but Workspace Catalog migration, SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success

### Requirement: Workspace Setup uses domain-native catalog data

Workspace Setup SHALL parse, validate, provide, persist, and expose domain-native Workflow Catalog and Workspace models without a separate workspace application contract layer or duplicated command model graph.

#### Scenario: Bundled Workflow Catalog is read

- **WHEN** Workspace Setup reads the bundled Workflow Catalog
- **THEN** the bundled catalog reader SHALL return domain-native Workflow Catalog data
- **AND** the catalog reader MUST NOT return `workspace_contracts.rs` DTOs
- **AND** the command boundary SHALL expose generated TypeScript bindings for returned domain-native Workflow Catalog data without requiring duplicated runtime command DTO graphs

#### Scenario: Workspace catalog is read

- **WHEN** Workspace Setup reads the local Workspace Catalog
- **THEN** the workspace repository SHALL return domain-native Workspace Catalog data
- **AND** the command boundary SHALL expose the domain-native Workspace Catalog through a generated command response wrapper
- **AND** the command boundary MAY use command-owned remote generated binding metadata for nested Workspace Catalog domain types
- **AND** returned Workspace records SHALL expose provider-discriminated domain Placement Plan data in the generated command payload
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation

#### Scenario: Workspace creation receives a placement plan

- **WHEN** the Client submits a Workspace creation request
- **THEN** the generated command request SHALL require provider-discriminated domain Placement Plan data without selected Provisioning Profile or Endpoint Profile data
- **AND** the submitted Placement Plan SHALL include the nested `gpu_cloud_provider_id` discriminator required by the domain Placement Plan shape
- **AND** the command boundary SHALL pass the submitted domain Placement Plan into the Workspace Setup service input without a parallel command Placement Plan DTO
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation

### Requirement: Workspace Setup services use domain-native inputs and results

Workspace Setup services SHALL use domain-native inputs and results for workflow catalogs, provider inventory, placement plans, and workspaces.

#### Scenario: Provider inventory is fetched

- **WHEN** Workspace Setup fetches provider inventory for a GPU Cloud Provider
- **THEN** the service SHALL pass a domain `GpuCloudProviderId` to the provider inventory gateway
- **AND** the service SHALL return domain provider inventory data to the command boundary

#### Scenario: Workspace is created

- **WHEN** Workspace Setup creates a Draft Workspace
- **THEN** the service SHALL receive a domain `GpuCloudProviderId` and provider-discriminated domain Placement Plan
- **AND** the service SHALL return a domain Workspace
- **AND** the service MUST NOT convert between domain Placement Plan data and duplicated workspace service DTOs

### Requirement: Validate Placement Plan against bundled catalogs

The Native Layer SHALL treat the bundled Workflow Catalog as authoritative when validating the provider-discriminated Placement Plan submitted by the Client.

#### Scenario: Submitted Workflow Preset matches bundled definition

- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset matches the bundled definition by id and content
- **THEN** the Native Layer SHALL accept that Workflow Preset for Workspace creation validation
- **AND** the Native Layer SHALL persist the selected Workflow Preset as a creation-time Workspace snapshot

#### Scenario: Submitted Workflow Preset is missing or stale

- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset does not exist in the bundled Workflow Catalog or does not match the bundled definition for its id
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Provider-discriminated placement is invalid

- **WHEN** the Client submits a Placement Plan whose provider variant does not match the submitted GPU Cloud Provider id
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Requested storage is below workflow minimum

- **WHEN** the Client submits a Placement Plan whose requested Persistent Storage Volume size is smaller than the selected Workflow Preset minimum
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

### Requirement: Keep bundled catalog validation offline

Bundled catalog validation SHALL validate local contract shape and safety constraints only.

#### Scenario: External resources are not checked during catalog validation

- **WHEN** the Native Layer validates the bundled Workflow Catalog
- **THEN** the Native Layer MUST NOT call Docker registries, Git repositories, Hugging Face, RunPod, worker HTTP endpoints, or any external service to validate reachability, existence, authenticity, or current availability
- **AND** external availability failures SHALL remain the responsibility of later provisioning or provider operations

### Requirement: Workspace creation reports placement validation failures precisely

Workspace creation SHALL return UI-safe placement validation categories for incomplete, stale, or incompatible Placement Plans.

#### Scenario: Placement provider does not match request provider

- **WHEN** the Placement Plan provider does not match the requested GPU Cloud Provider
- **THEN** the Native Layer SHALL reject the request with a placement provider mismatch error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement selection is incomplete

- **WHEN** the Placement Plan is missing a selected datacenter or selected GPU
- **THEN** the Native Layer SHALL reject the request with a field-specific placement selection error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement references stale Workflow Preset data

- **WHEN** the selected Workflow Preset is absent from current bundled Workflow Catalog data or does not exactly match current bundled Workflow Catalog data
- **THEN** the Native Layer SHALL reject the request with a stale catalog object error identifying the Workflow Preset
- **AND** React SHALL be able to prompt the user to reload catalogs and reselect placement data

#### Scenario: Requested storage is below preset minimum

- **WHEN** the requested persistent storage volume size is smaller than the selected Workflow Preset required base volume size
- **THEN** the Native Layer SHALL reject the request with a storage minimum error
- **AND** React SHALL be able to identify that storage selection must be increased

## REMOVED Requirements

### Requirement: Read bundled Provisioning Profiles

**Reason**: Provisioning Profiles are removed as a concept because v1 uses one standardized Provisioner Worker runtime owned by Native configuration.

**Migration**: Remove `get_provisioning_profiles` and all frontend/native uses. Workspace Setup no longer loads or validates provisioning profile catalog data.

### Requirement: Read bundled Endpoint Profiles

**Reason**: Endpoint Profiles are removed as a concept because v1 uses one standardized Endpoint Worker runtime owned by Native configuration.

**Migration**: Remove `get_endpoint_profiles` and all frontend/native uses. Workspace Setup no longer loads or validates endpoint profile catalog data.

### Requirement: Validate Profile catalog surface fields

**Reason**: Profile catalog files and profile runtime/provider fields are removed.

**Migration**: Move worker image refs and ports to build-time Native app configuration. Remove fixed mount path, cloud type, and container disk size from Workspace Setup profile contracts; reintroduce them as provider-owned implementation details when provisioning code needs them.

### Requirement: Use docker image refs without digest metadata

**Reason**: Docker image refs are no longer represented on profile worker runtime objects.

**Migration**: Require Provisioner Worker and Endpoint Worker image refs through Native build configuration instead of bundled profile catalogs.

### Requirement: Workspace Setup read commands report source-specific catalog failures

**Reason**: Workspace Setup no longer has separate Provisioning Profile or Endpoint Profile read commands.

**Migration**: Keep Workflow Catalog read failures on the existing Workflow Catalog command error path.
