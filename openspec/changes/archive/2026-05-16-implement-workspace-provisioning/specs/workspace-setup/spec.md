## MODIFIED Requirements

### Requirement: Read provider placement inventory

The Native Layer SHALL expose a `get_provider_placement_options` command that returns provider placement options for an explicit GPU Cloud Provider after validating local provider setup prerequisites. Placement options SHALL include live Provider Inventory and provider placement capabilities.

#### Scenario: Provider setup is complete

- **WHEN** the Client requests provider placement options for `runpod` and the local Provider API Key exists
- **THEN** the Native Layer SHALL call RunPod using the stored Provider API Key to fetch live Provider Inventory
- **AND** the Native Layer SHALL return available datacenters, GPU options per datacenter, and provider maximum Persistent Storage Volume size when known
- **AND** the Native Layer SHALL return placement capabilities for RunPod endpoint keep-alive with `supported = true`, `default_seconds = 5`, `min_seconds = 5`, and `max_seconds = 3600`
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Provider setup is incomplete

- **WHEN** the Client requests provider placement options and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST reject before calling the Provider

#### Scenario: Provider API Key is invalid or revoked

- **WHEN** the Client requests provider placement options and the Provider rejects the stored Provider API Key as unauthorized or forbidden
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT report the failure as retryable
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory lookup fails

- **WHEN** the Provider inventory request fails due to timeout, transport error, unavailable Provider API, or unreadable provider response
- **THEN** the Native Layer SHALL reject the request with `provider_api_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider does not support endpoint keep-alive

- **WHEN** a future GPU Cloud Provider does not support endpoint keep-alive configuration
- **THEN** its provider placement options SHALL return endpoint keep-alive capability with `supported = false`
- **AND** that provider's Placement Plan variant MUST NOT persist an endpoint keep-alive value

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record after required Workspace Catalog persistence migrations have completed.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid RunPod Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled Workflow Preset compatibility, placement structure, and RunPod endpoint keep-alive range before persistence
- **AND** the RunPod Placement Plan SHALL include provider-specific endpoint keep-alive seconds
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

Workspace Setup SHALL parse, validate, provide, persist, and expose domain-native Workflow Catalog, provider placement options, and Workspace models without a separate workspace application contract layer or duplicated command model graph.

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

#### Scenario: Provider placement options are read

- **WHEN** Workspace Setup reads provider placement options
- **THEN** the service SHALL return domain-native Provider Inventory data and provider placement capability data to the command boundary
- **AND** the command boundary SHALL expose generated TypeScript bindings for the placement options response
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation

#### Scenario: Workspace creation receives a placement plan

- **WHEN** the Client submits a Workspace creation request
- **THEN** the generated command request SHALL require provider-discriminated domain Placement Plan data without selected Provisioning Profile or Endpoint Profile data
- **AND** the submitted RunPod Placement Plan SHALL include provider-specific endpoint keep-alive seconds
- **AND** the submitted Placement Plan SHALL include the nested `gpu_cloud_provider_id` discriminator required by the domain Placement Plan shape
- **AND** the command boundary SHALL pass the submitted domain Placement Plan into the Workspace Setup service input without a parallel command Placement Plan DTO
- **AND** Workspace Setup domain models MUST NOT derive `specta::Type` solely to satisfy generated command payload generation

### Requirement: Workspace Setup services use domain-native inputs and results

Workspace Setup services SHALL use domain-native inputs and results for workflow catalogs, provider placement options, placement plans, and workspaces.

#### Scenario: Provider placement options are fetched

- **WHEN** Workspace Setup fetches provider placement options for a GPU Cloud Provider
- **THEN** the service SHALL pass a domain `GpuCloudProviderId` to the provider inventory gateway
- **AND** the service SHALL return domain provider inventory data and provider placement capabilities to the command boundary

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

#### Scenario: RunPod endpoint keep-alive is outside provider range

- **WHEN** the Client submits a RunPod Placement Plan whose endpoint keep-alive seconds is lower than `5` or greater than `3600`
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

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

#### Scenario: RunPod endpoint keep-alive is invalid

- **WHEN** the requested RunPod endpoint keep-alive seconds is outside the provider-supported range
- **THEN** the Native Layer SHALL reject the request with a placement endpoint keep-alive range error
- **AND** React SHALL be able to identify that endpoint keep-alive selection must be changed
