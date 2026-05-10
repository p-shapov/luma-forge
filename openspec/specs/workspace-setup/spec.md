# workspace-setup Specification

## Purpose
Define the native-owned Workspace Setup lifecycle for reading bundled catalogs, validating placement plans, and creating local draft Workspace records without creating provider resources or exposing provider secrets.
## Requirements
### Requirement: Read bundled Workflow Catalog

The Native Layer SHALL expose a command that returns the bundled Workflow Catalog available in the current application build. Every Workflow Preset declared by the bundled Workflow Catalog SHALL satisfy the Native Layer's offline surface validation before any catalog data is exposed or accepted.

#### Scenario: Workflow Catalog is available

- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Catalog containing selectable Workflow Presets
- **AND** every returned model asset SHALL include an explicit ComfyUI-relative install path
- **AND** every returned Custom Node SHALL include a safe ComfyUI-relative checkout path under `custom_nodes/...`
- **AND** every returned Custom Node SHALL represent requirements installation as an optional checkout-root-relative path
- **AND** the response MUST NOT read or mutate the Workspace Catalog

#### Scenario: Workflow Catalog is unavailable or invalid

- **WHEN** the Client requests the Workflow Catalog and the bundled catalog is unavailable, unreadable, empty, internally inconsistent, or contains unsafe or malformed Workflow Preset surface data
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

The Native Layer SHALL expose a command that returns the bundled Provisioning Profiles available in the current application build. Every bundled Provisioning Profile SHALL satisfy offline surface validation before it is exposed or accepted.

#### Scenario: Provisioning Profiles are available

- **WHEN** the Client requests Provisioning Profiles
- **THEN** the Native Layer SHALL return the available Provisioning Profiles
- **AND** every returned Provisioning Profile SHALL include only UI-safe configuration data

#### Scenario: Provisioning Profiles are unavailable or invalid

- **WHEN** the Client requests Provisioning Profiles and the bundled profile catalog is unavailable, unreadable, empty, internally inconsistent, or contains unsafe or malformed Provisioning Profile surface data
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Read bundled Endpoint Profiles

The Native Layer SHALL expose a command that returns the bundled Endpoint Profiles available in the current application build. Every bundled Endpoint Profile SHALL satisfy offline surface validation before it is exposed or accepted.

#### Scenario: Endpoint Profiles are available

- **WHEN** the Client requests Endpoint Profiles
- **THEN** the Native Layer SHALL return the available Endpoint Profiles
- **AND** every returned Endpoint Profile SHALL include only UI-safe configuration data

#### Scenario: Endpoint Profiles are unavailable or invalid

- **WHEN** the Client requests Endpoint Profiles and the bundled profile catalog is unavailable, unreadable, empty, internally inconsistent, or contains unsafe or malformed Endpoint Profile surface data
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Read Workspace Catalog

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL verify that each returned persisted Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, migration, read, decoding, or row consistency validation fails
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

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model before the record is mapped to the serializable Workspace Catalog shape.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled catalog compatibility, profile compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL construct the Draft Workspace through the domain Workspace model
- **AND** the domain-authored Workspace SHALL have lifecycle state `draft`
- **AND** the domain-authored Workspace SHALL have empty Persistent Storage Volume, active Provisioning Pod, Serverless Endpoint, and last Provisioning Pod snapshots
- **AND** the Native Layer SHALL map the domain-authored Workspace to the serializable Workspace Catalog record
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

### Requirement: Preserve local provider key prerequisite during Workspace creation

The Native Layer SHALL prevent provider setup deletion from interleaving with Workspace creation for the same GPU Cloud Provider between local Provider API Key validation and Draft Workspace persistence.

#### Scenario: Workspace creation and provider setup deletion are serialized

- **WHEN** Workspace creation for `runpod` starts while provider setup deletion for `runpod` is evaluating or mutating local setup state
- **THEN** Workspace creation SHALL evaluate local Provider API Key presence only after the delete operation has finished
- **AND** Workspace creation SHALL reject with `provider_setup_incomplete` if the required local Provider API Key is missing
- **AND** Workspace creation MUST NOT persist a Workspace record when the local Provider API Key prerequisite is missing

#### Scenario: Provider setup deletion waits for Workspace creation persistence

- **WHEN** provider setup deletion for `runpod` starts while Workspace creation for `runpod` is validating provider setup and persisting a Draft Workspace
- **THEN** provider setup deletion SHALL wait until Workspace creation has either persisted and re-read the Workspace record or failed
- **AND** Workspace creation SHALL persist only after confirming the local Provider API Key prerequisite inside the serialized operation

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

### Requirement: Validate Workflow Preset source fields

The Native Layer SHALL validate bundled Workflow Preset source fields using offline surface checks before exposing or accepting the Workflow Preset.

#### Scenario: Workflow Preset source fields are valid

- **WHEN** a bundled Workflow Preset declares a URL-shaped ComfyUI Git repository URL, a non-empty ComfyUI revision, and model assets with non-empty Hugging Face repository ids, safe repo-relative file paths, and non-empty revisions
- **THEN** the Native Layer SHALL treat those source fields as valid catalog data
- **AND** the Native Layer SHALL NOT call Git, Hugging Face, or any network service to verify resource existence

#### Scenario: Workflow Preset source fields are invalid

- **WHEN** a bundled Workflow Preset declares a blank or non-URL-shaped ComfyUI Git repository URL, a blank ComfyUI revision, a malformed Hugging Face repository id, an unsafe model source file path, or a blank model source revision
- **THEN** the Native Layer SHALL treat the bundled Workflow Catalog as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation with `workflow_catalog_unavailable`

### Requirement: Validate Custom Node catalog entries

The Native Layer SHALL validate every bundled Custom Node entry before exposing or accepting the Workflow Preset that contains it.

#### Scenario: Custom Node catalog entry is valid

- **WHEN** a bundled Custom Node declares non-empty id and name values, a URL-shaped Git repository URL, a non-empty revision, a safe checkout path under `custom_nodes/...`, and no requirements path
- **THEN** the Native Layer SHALL treat the Custom Node as valid catalog data
- **AND** the absence of a requirements path SHALL mean dependency installation is skipped for that Custom Node

#### Scenario: Custom Node requirements path is valid

- **WHEN** a bundled Custom Node declares a requirements path
- **THEN** the Native Layer SHALL require that path to be non-empty, relative, normalized, and free of current-directory, empty, absolute, and parent-traversal segments
- **AND** the Native Layer SHALL treat the path as relative to the Custom Node checkout root

#### Scenario: Custom Node catalog entry is invalid

- **WHEN** a bundled Custom Node declares a blank id, blank name, blank or non-URL-shaped Git repository URL, blank revision, unsafe checkout path, checkout path outside `custom_nodes/...`, or unsafe requirements path
- **THEN** the Native Layer SHALL treat the bundled Workflow Catalog as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation with `workflow_catalog_unavailable`

### Requirement: Validate Profile catalog surface fields

The Native Layer SHALL validate bundled Provisioning Profile and Endpoint Profile runtime/provider fields using offline surface checks before exposing or accepting those profiles.

#### Scenario: Profile catalog surface fields are valid

- **WHEN** a bundled profile declares non-empty ids, versions, names, worker versions, plausible Docker image refs, absolute normalized POSIX mount paths other than `/`, valid nonzero ports, HTTP paths that start with `/` and contain no query or fragment, supported enum-like values, and internally consistent scaling values
- **THEN** the Native Layer SHALL treat the profile as valid catalog data
- **AND** the Native Layer SHALL NOT call Docker registries, Provider APIs, or worker endpoints to verify resource existence or availability

#### Scenario: Profile catalog surface fields are invalid

- **WHEN** a bundled profile declares a blank required field, malformed Docker image ref, relative mount path, root-only mount path, path with traversal, invalid port, malformed HTTP path, unsupported enum-like value, or inconsistent scaling values
- **THEN** the Native Layer SHALL treat the affected bundled profile catalog as invalid
- **AND** the Native Layer SHALL reject the corresponding profile read and Workspace creation with `workflow_catalog_unavailable`

### Requirement: Use docker image refs without digest metadata

The Native Layer SHALL represent v1 worker Docker images using `docker_image_ref` directly on worker runtime objects.

#### Scenario: Docker image ref is accepted

- **WHEN** a bundled Provisioning Profile or Endpoint Profile worker runtime declares a plausible non-empty `docker_image_ref`
- **THEN** the Native Layer SHALL treat the Docker image identity as valid catalog data
- **AND** the Native Layer SHALL NOT require `docker_image_digest`

#### Scenario: Docker image wrapper and digest are not part of the contract

- **WHEN** the Native Layer exposes generated frontend bindings, reference contracts, domain snapshots, or Workspace records containing Docker image metadata
- **THEN** those contracts SHALL NOT include `docker_image_digest`
- **AND** those contracts SHALL NOT wrap `docker_image_ref` in a one-field Docker image object
- **AND** the Native Layer SHALL NOT imply Docker image authenticity or digest pinning during Workspace Setup

### Requirement: Keep bundled catalog validation offline

Bundled catalog validation SHALL validate local contract shape and safety constraints only.

#### Scenario: External resources are not checked during catalog validation

- **WHEN** the Native Layer validates bundled Workflow Catalogs, Provisioning Profiles, or Endpoint Profiles
- **THEN** the Native Layer MUST NOT call Docker registries, Git repositories, Hugging Face, RunPod, worker HTTP endpoints, or any external service to validate reachability, existence, authenticity, or current availability
- **AND** external availability failures SHALL remain the responsibility of later provisioning or provider operations
