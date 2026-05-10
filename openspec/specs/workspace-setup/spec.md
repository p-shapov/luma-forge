# workspace-setup Specification

## Purpose
Define the native-owned Workspace Setup lifecycle for reading bundled catalogs, validating placement plans, and creating local draft Workspace records without creating provider resources or exposing provider secrets.

## Requirements

### Requirement: Read bundled Workflow Catalog

The Native Layer SHALL expose a command that returns the bundled Workflow Catalog available in the current application build. Every model asset declared by a bundled Workflow Preset SHALL include an explicit ComfyUI-relative install path used by the Provisioner Worker.

#### Scenario: Workflow Catalog is available

- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Catalog containing selectable Workflow Presets
- **AND** every returned model asset SHALL include an explicit ComfyUI-relative install path
- **AND** the response MUST NOT read or mutate the Workspace Catalog

#### Scenario: Workflow Catalog is unavailable or invalid

- **WHEN** the Client requests the Workflow Catalog and the bundled catalog is unavailable, unreadable, empty, internally inconsistent, or contains a model asset without a safe explicit install path
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Validate model asset install paths

The Native Layer SHALL validate bundled Workflow Preset model asset install paths before exposing or accepting the Workflow Preset.

#### Scenario: Model asset install path is safe

- **WHEN** a bundled Workflow Preset declares a model asset install path that is non-empty, relative, normalized, and does not contain parent traversal
- **THEN** the Native Layer SHALL treat the install path as valid catalog data
- **AND** Workspace Setup validation MAY accept the Workflow Preset when all other catalog rules pass

#### Scenario: Model asset install path is unsafe

- **WHEN** a bundled Workflow Preset declares a model asset install path that is blank, absolute, contains parent traversal, or cannot be safely resolved under a ComfyUI root
- **THEN** the Native Layer SHALL treat the bundled Workflow Catalog as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation with `workflow_catalog_unavailable`

### Requirement: Read bundled Provisioning Profiles

The Native Layer SHALL expose a command that returns the bundled Provisioning Profiles available in the current application build.

#### Scenario: Provisioning Profiles are available

- **WHEN** the Client requests Provisioning Profiles
- **THEN** the Native Layer SHALL return the available Provisioning Profiles
- **AND** every returned Provisioning Profile SHALL include only UI-safe configuration data

#### Scenario: Provisioning Profiles are unavailable or invalid

- **WHEN** the Client requests Provisioning Profiles and the bundled profile catalog is unavailable, unreadable, empty, or internally inconsistent
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Read bundled Endpoint Profiles

The Native Layer SHALL expose a command that returns the bundled Endpoint Profiles available in the current application build.

#### Scenario: Endpoint Profiles are available

- **WHEN** the Client requests Endpoint Profiles
- **THEN** the Native Layer SHALL return the available Endpoint Profiles
- **AND** every returned Endpoint Profile SHALL include only UI-safe configuration data

#### Scenario: Endpoint Profiles are unavailable or invalid

- **WHEN** the Client requests Endpoint Profiles and the bundled profile catalog is unavailable, unreadable, empty, or internally inconsistent
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Read Workspace Catalog

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, migration, read, or decoding fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

### Requirement: Read provider placement inventory

The Native Layer SHALL expose a command that returns placement inventory for an explicit GPU Cloud Provider after validating local provider setup prerequisites.

#### Scenario: Provider setup is complete

- **WHEN** the Client requests provider inventory for `runpod` and the local Provider API Key exists
- **THEN** the Native Layer SHALL call RunPod using the stored Provider API Key
- **AND** the Native Layer SHALL return available datacenters, GPU options per datacenter, and provider maximum Persistent Storage Volume size when known
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Provider setup is incomplete

- **WHEN** the Client requests provider inventory and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST reject before calling the Provider

#### Scenario: Provider API Key is invalid or revoked

- **WHEN** the Client requests provider inventory and the Provider rejects the stored Provider API Key as unauthorized or forbidden
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT report the failure as retryable
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory lookup fails

- **WHEN** the Provider inventory request fails due to timeout, transport error, unavailable Provider API, or unreadable provider response
- **THEN** the Native Layer SHALL reject the request with `provider_api_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate provider setup, bundled catalog compatibility, profile compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL persist one Workspace Catalog entry in SQLite with lifecycle state `draft`
- **AND** the Native Layer SHALL re-read the persisted Workspace record from SQLite
- **AND** the Native Layer SHALL return the re-read Workspace record as authoritative

#### Scenario: Duplicate Workspace UUID

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record
- **AND** the Native Layer MUST NOT create a second Workspace record for the same Workspace UUID

#### Scenario: Provider setup is missing during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace Catalog write fails

- **WHEN** the Client submits a valid Workspace creation request but SQLite write, commit, or re-read fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success

### Requirement: Preserve provider setup prerequisite during Workspace creation

The Native Layer SHALL prevent provider setup deletion from interleaving with Workspace creation for the same GPU Cloud Provider between provider setup validation and Draft Workspace persistence.

#### Scenario: Workspace creation and provider setup deletion are serialized

- **WHEN** Workspace creation for `runpod` starts while provider setup deletion for `runpod` is evaluating or mutating local setup state
- **THEN** Workspace creation SHALL evaluate provider setup completeness only after the delete operation has finished
- **AND** Workspace creation SHALL reject with `provider_setup_incomplete` if the required local Provider API Key is missing
- **AND** Workspace creation MUST NOT persist a Workspace record when provider setup is incomplete

#### Scenario: Provider setup deletion waits for Workspace creation persistence

- **WHEN** provider setup deletion for `runpod` starts while Workspace creation for `runpod` is validating provider setup and persisting a Draft Workspace
- **THEN** provider setup deletion SHALL wait until Workspace creation has either persisted and re-read the Workspace record or failed
- **AND** Workspace creation SHALL persist only after confirming provider setup is complete inside the serialized operation

#### Scenario: Workspace duplicate handling remains database-owned

- **WHEN** two Workspace creation requests use the same Workspace UUID concurrently
- **THEN** the Native Layer SHALL rely on the Workspace Catalog uniqueness boundary to persist at most one Workspace record for that UUID
- **AND** the losing request SHALL reject with `workspace_already_exists`
- **AND** provider setup serialization MUST NOT replace SQLite uniqueness enforcement

### Requirement: Validate Placement Plan against bundled catalogs

The Native Layer SHALL treat the bundled Workflow Catalog, Provisioning Profiles, and Endpoint Profiles as authoritative when validating the full Placement Plan submitted by the Client.

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

#### Scenario: Requested storage is below workflow minimum

- **WHEN** the Client submits a Placement Plan whose requested Persistent Storage Volume size is smaller than the selected Workflow Preset minimum
- **THEN** the Native Layer SHALL reject the Workspace creation request with `invalid_placement_plan`
- **AND** the Native Layer MUST NOT persist a Workspace record

### Requirement: Do not validate live availability during Workspace creation

Workspace creation SHALL validate structural Placement Plan completeness and compatibility, but MUST NOT require the selected GPU or datacenter to still be live-available at creation time.

#### Scenario: Selected GPU was returned by a previous inventory lookup

- **WHEN** the Client submits a structurally valid Placement Plan after provider inventory may have changed
- **THEN** the Native Layer SHALL NOT reject the Workspace creation request solely because live Provider inventory no longer reports availability for the selected GPU or datacenter
- **AND** final Provider availability handling SHALL remain the responsibility of Workspace Provisioning

### Requirement: Create no Provider Resources during Workspace Setup

Workspace Setup MUST NOT create, modify, attach, or delete Provider Resources.

#### Scenario: Workspace is created successfully

- **WHEN** the Native Layer successfully creates a `draft` Workspace
- **THEN** the persisted Workspace SHALL have empty Persistent Storage Volume, active Provisioning Pod, and Serverless Endpoint snapshots
- **AND** the Native Layer MUST NOT create a Persistent Storage Volume, Provisioning Pod, or Serverless Endpoint

#### Scenario: Workspace creation fails

- **WHEN** Workspace creation validation or persistence fails
- **THEN** the Native Layer MUST NOT perform Provider Resource cleanup
- **AND** the Native Layer MUST NOT have created Provider Resources for that failed Workspace Setup attempt

### Requirement: Keep Provider API Key secret during Workspace Setup

Workspace Setup commands MUST NOT return, persist outside secure keyring, log, or include Provider API Keys in generated frontend types, command responses, errors, Workspace records, Provider Inventory responses, or diagnostics.

#### Scenario: Provider inventory is returned

- **WHEN** the Native Layer returns Provider Inventory
- **THEN** the response SHALL include only UI-safe provider inventory data
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Workspace is created

- **WHEN** the Native Layer persists or returns a Workspace record
- **THEN** the Workspace record MUST NOT include the Provider API Key
- **AND** the Workspace record SHALL reference only the GPU Cloud Provider id and UI-safe selected configuration snapshots
