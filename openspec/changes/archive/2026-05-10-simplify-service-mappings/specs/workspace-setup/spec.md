## MODIFIED Requirements

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled catalog compatibility, profile compatibility, and placement structure before persistence
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
- **THEN** the Native Layer SHALL reject the request with `workspace_already_exists`
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

- **WHEN** the Client submits a valid Workspace creation request but SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success

### Requirement: Validate Placement Plan against bundled catalogs

The Native Layer SHALL treat the bundled Workflow Catalog, Provisioning Profiles, and Endpoint Profiles as authoritative when validating the full provider-discriminated Placement Plan submitted by the Client.

#### Scenario: Submitted catalog objects match bundled definitions

- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset, Provisioning Profile, and Endpoint Profile match bundled definitions by id and content
- **THEN** the Native Layer SHALL accept those objects for Workspace creation validation
- **AND** the Native Layer SHALL persist those selected objects as creation-time Workspace snapshots

#### Scenario: Submitted catalog object is missing or stale

- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset, Provisioning Profile, or Endpoint Profile does not exist in bundled catalogs or does not match the bundled definition for its id
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Profile compatibility is invalid

- **WHEN** the Client submits a Placement Plan whose selected profiles are incompatible with the selected Workflow Preset or GPU Cloud Provider
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Provider-discriminated placement is invalid

- **WHEN** the Client submits a Placement Plan whose provider variant does not match the submitted GPU Cloud Provider id or whose nested profile variants do not match the Placement Plan provider variant
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Requested storage is below workflow minimum

- **WHEN** the Client submits a Placement Plan whose requested Persistent Storage Volume size is smaller than the selected Workflow Preset minimum
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

## ADDED Requirements

### Requirement: Workspace Setup uses domain-native catalog data

Workspace Setup SHALL parse, validate, provide, and persist domain-native catalog and workspace models without a separate workspace application contract layer duplicating domain types.

#### Scenario: Bundled catalogs are read

- **WHEN** Workspace Setup reads bundled Workflow Catalog, Provisioning Profiles, or Endpoint Profiles
- **THEN** the bundled catalog reader SHALL return domain-native catalog and profile data
- **AND** the catalog reader MUST NOT return `workspace_contracts.rs` DTOs

#### Scenario: Workspace catalog is read

- **WHEN** Workspace Setup reads the local Workspace Catalog
- **THEN** the workspace repository SHALL return domain-native Workspace Catalog data
- **AND** the command boundary SHALL map the domain catalog into generated command response DTOs before returning it to React

### Requirement: Workspace Setup services use domain-native inputs and results

Workspace Setup services SHALL use domain-native inputs and results for workflow catalogs, profiles, provider inventory, placement plans, and workspaces.

#### Scenario: Provider inventory is fetched

- **WHEN** Workspace Setup fetches provider inventory for a GPU Cloud Provider
- **THEN** the service SHALL pass a domain `GpuCloudProviderId` to the provider inventory gateway
- **AND** the service SHALL return domain provider inventory data to the command boundary

#### Scenario: Workspace is created

- **WHEN** Workspace Setup creates a Draft Workspace
- **THEN** the service SHALL receive a domain `GpuCloudProviderId` and provider-discriminated domain Placement Plan
- **AND** the service SHALL return a domain Workspace
- **AND** the service MUST NOT convert between domain Placement Plan data and duplicated workspace service DTOs
