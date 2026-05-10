# workspace-setup Specification

## Purpose
Define the native-owned Workspace Setup lifecycle for reading bundled catalogs, validating placement plans, and creating local draft Workspace records without creating provider resources or exposing provider secrets.
## Requirements
### Requirement: Read bundled Workflow Catalog
The Native Layer SHALL expose a command that returns the bundled Workflow Catalog available in the current application build. Every Workflow Preset declared by the bundled Workflow Catalog SHALL satisfy the Native Layer's offline surface validation before any catalog data is exposed or accepted.

#### Scenario: Workflow Catalog is available
- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Catalog containing selectable Workflow Presets
- **AND** every returned model asset SHALL include public Hugging Face download metadata with repository id, file path, revision, and explicit ComfyUI-relative install path
- **AND** every returned model asset MUST NOT include extra app-owned asset metadata
- **AND** every returned ComfyUI and Custom Node Git source SHALL include an immutable commit revision for worker-prepared checkout
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

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog after required Workspace Catalog persistence migrations have completed.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL apply required Workspace Catalog persistence migrations before decoding rows
- **AND** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL verify that each returned persisted Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, migration, read, decoding, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

### Requirement: Migrate Workspace Catalog persistence before use

The Native Layer SHALL apply Workspace Catalog SQLite schema and persisted Workspace JSON migrations before using the Workspace Catalog for reads, duplicate checks, inserts, or post-insert re-reads.

#### Scenario: Unversioned Workspace Catalog is migrated

- **WHEN** the Native Layer opens an existing Workspace Catalog that has no recorded persistence version
- **THEN** the Native Layer SHALL treat the catalog as version `0`
- **AND** the Native Layer SHALL apply every required migration up to the current persistence version before returning or writing Workspace records
- **AND** the Native Layer SHALL record the current persistence version only after all required migrations complete successfully

#### Scenario: Legacy Workspace JSON is compatible with current bundled catalogs

- **WHEN** a persisted Workspace row contains legacy embedded Workflow Preset, Provisioning Profile, or Endpoint Profile JSON whose selected ids still exist in the current bundled catalogs
- **THEN** the Native Layer SHALL migrate the persisted Workspace JSON into the current domain shape using the current bundled catalog/profile definitions for those selected ids
- **AND** the Native Layer SHALL preserve the Workspace id, name, GPU Cloud Provider id, lifecycle state, selected datacenter, selected GPU, requested persistent storage size, Provider Resource snapshots, and environment preparation timestamp
- **AND** the Native Layer SHALL validate the migrated Workspace record before making it visible as authoritative Workspace Catalog data

#### Scenario: Legacy Workspace JSON cannot be migrated

- **WHEN** a persisted Workspace row cannot be migrated because required selected catalog/profile ids are missing, JSON is malformed, row data is inconsistent, or the migrated Workspace fails domain validation
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative
- **AND** the Native Layer MUST NOT mark the failed migration version as applied

#### Scenario: Current Workspace Catalog is already migrated

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version matches the current application persistence version
- **THEN** the Native Layer SHALL use the existing Workspace records without rewriting them for migration
- **AND** normal Workspace Catalog read, duplicate check, insert, and row consistency validation rules SHALL still apply

#### Scenario: Workspace Catalog was written by a newer app version

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version is greater than the current application persistence version
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT read, write, migrate, downgrade, or mutate Workspace records from the newer catalog version

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

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record after required Workspace Catalog persistence migrations have completed.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled catalog compatibility, profile compatibility, and placement structure before persistence
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

### Requirement: Validate worker-prepared Git source revisions
The Native Layer SHALL validate bundled Workflow Preset Git revisions for sources that the Provisioner Worker prepares remotely.

#### Scenario: Worker-prepared Git revisions are immutable
- **WHEN** a bundled Workflow Preset declares ComfyUI or Custom Node Git sources with full 40-character lowercase hexadecimal commit revisions
- **THEN** the Native Layer SHALL treat those revisions as valid catalog data
- **AND** Workspace Setup validation MAY accept the Workflow Preset when all other catalog rules pass

#### Scenario: Worker-prepared Git revision is mutable
- **WHEN** a bundled Workflow Preset declares a ComfyUI or Custom Node Git source revision as a branch name, tag name, blank value, or non-commit value
- **THEN** the Native Layer SHALL treat the bundled Workflow Catalog as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation with `workflow_catalog_unavailable`

### Requirement: Validate Hugging Face model download metadata
The Native Layer SHALL validate bundled Workflow Preset Hugging Face model asset metadata required for public model downloads.

#### Scenario: Hugging Face model metadata is valid
- **WHEN** a bundled Workflow Preset declares a model asset with a Hugging Face repository id, file path, non-empty revision, and explicit ComfyUI-relative install path
- **THEN** the Native Layer SHALL treat the model asset as valid catalog data
- **AND** Workspace Setup validation MAY accept the Workflow Preset when all other catalog rules pass

#### Scenario: Hugging Face model metadata is invalid
- **WHEN** a bundled Workflow Preset declares a model asset with a blank repository id, blank file path, blank revision, unsupported source type, or unsafe install path
- **THEN** the Native Layer SHALL treat the bundled Workflow Catalog as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation with `workflow_catalog_unavailable`

#### Scenario: Model asset has no extra app-owned metadata
- **WHEN** a bundled Workflow Preset declares public Hugging Face model asset metadata
- **THEN** the Native Layer SHALL NOT require fields beyond the Hugging Face source metadata and install path
- **AND** the Native Layer SHALL NOT expose extra model asset metadata through generated command bindings

### Requirement: Workspace Setup read commands report source-specific catalog failures

Workspace Setup read commands SHALL report which local catalog or profile source failed instead of collapsing every bundled catalog/profile failure into Workflow Catalog unavailability.

#### Scenario: Workflow Catalog read fails

- **WHEN** the Native Layer cannot parse, validate, or return the bundled Workflow Catalog
- **THEN** `get_workflow_catalog` SHALL reject with `workflow_catalog_unavailable` or a more specific Workflow Catalog invalid/unavailable code

#### Scenario: Provisioning Profiles read fails

- **WHEN** the Native Layer cannot parse, validate, or return bundled Provisioning Profiles
- **THEN** `get_provisioning_profiles` SHALL reject with a Provisioning Profiles-specific UI-safe code
- **AND** the command MUST NOT return `workflow_catalog_unavailable` solely because Provisioning Profiles failed

#### Scenario: Endpoint Profiles read fails

- **WHEN** the Native Layer cannot parse, validate, or return bundled Endpoint Profiles
- **THEN** `get_endpoint_profiles` SHALL reject with an Endpoint Profiles-specific UI-safe code
- **AND** the command MUST NOT return `workflow_catalog_unavailable` solely because Endpoint Profiles failed

### Requirement: Provider Inventory reports provider failure classes precisely

Provider Inventory reads SHALL distinguish Provider authorization, provider network/API availability, malformed responses, and invalid mapped inventory.

#### Scenario: Stored Provider API Key is missing

- **WHEN** the Client requests Provider Inventory and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`

#### Scenario: Stored Provider API Key is unauthorized

- **WHEN** RunPod rejects the stored Provider API Key while fetching inventory
- **THEN** the Native Layer SHALL reject the request with a Provider API Key authorization error
- **AND** React SHALL be able to route the user toward Provider Setup recovery

#### Scenario: Provider Inventory request cannot reach provider

- **WHEN** RunPod inventory lookup fails due to timeout, DNS, connection failure, request timeout, provider outage, rate limiting, or non-auth provider availability failure
- **THEN** the Native Layer SHALL reject the request with a retryable provider availability error

#### Scenario: Provider Inventory response is malformed or invalid

- **WHEN** RunPod inventory lookup returns a response that cannot be parsed, mapped, or validated as a Provider Inventory
- **THEN** the Native Layer SHALL reject the request with a Provider response or inventory invalid error
- **AND** the generated command error MUST NOT include the raw Provider response body

### Requirement: Workspace creation reports request validation failures precisely

Workspace creation SHALL return field-specific UI-safe errors for invalid command request shape before evaluating provider setup, catalogs, placement, or persistence.

#### Scenario: Workspace UUID is invalid

- **WHEN** the Client submits a Workspace creation request whose `workspace_id` is missing or is not a valid UUID
- **THEN** the Native Layer SHALL reject the request with `invalid_workspace_id`
- **AND** the Native Layer MUST NOT read Provider setup, bundled catalogs, Provider Inventory, or Workspace Catalog persistence

#### Scenario: Workspace name is missing

- **WHEN** the Client submits a Workspace creation request whose `name` is empty or blank after trimming
- **THEN** the Native Layer SHALL reject the request with `workspace_name_required`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace metadata construction fails

- **WHEN** the Native Layer cannot construct a valid Draft Workspace from otherwise parsed request data
- **THEN** the Native Layer SHALL reject the request with `invalid_workspace_metadata`
- **AND** the Native Layer MUST NOT persist a Workspace record

### Requirement: Workspace creation reports placement validation failures precisely

Workspace creation SHALL return UI-safe placement validation categories for incomplete, stale, or incompatible Placement Plans.

#### Scenario: Placement provider does not match request provider

- **WHEN** the Placement Plan provider, selected Provisioning Profile provider, or selected Endpoint Profile provider does not match the requested GPU Cloud Provider
- **THEN** the Native Layer SHALL reject the request with a placement provider mismatch error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement selection is incomplete

- **WHEN** the Placement Plan is missing a selected datacenter or selected GPU
- **THEN** the Native Layer SHALL reject the request with a field-specific placement selection error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Placement references stale catalog data

- **WHEN** the selected Workflow Preset, Provisioning Profile, or Endpoint Profile is absent from current bundled catalog data or does not exactly match current bundled catalog data
- **THEN** the Native Layer SHALL reject the request with a stale catalog object error identifying the stale object category
- **AND** React SHALL be able to prompt the user to reload catalogs and reselect placement data

#### Scenario: Endpoint profile is incompatible with workflow

- **WHEN** the selected Endpoint Profile workflow execution type does not match the selected Workflow Preset workflow execution type
- **THEN** the Native Layer SHALL reject the request with an endpoint/profile compatibility error
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Requested storage is below preset minimum

- **WHEN** the requested persistent storage volume size is smaller than the selected Workflow Preset required base volume size
- **THEN** the Native Layer SHALL reject the request with a storage minimum error
- **AND** React SHALL be able to identify that storage selection must be increased

### Requirement: Workspace Catalog command errors distinguish safe recovery categories

Workspace Catalog read and write failures SHALL expose safe command-level categories that help React choose retry, recovery, or blocking behavior.

#### Scenario: Local storage path is unavailable

- **WHEN** the Native Layer cannot resolve or create the app data directory or connect to the SQLite catalog file
- **THEN** Workspace Catalog commands SHALL reject with a local storage or Workspace Catalog storage unavailable error

#### Scenario: Workspace Catalog migration fails

- **WHEN** Workspace Catalog initialization cannot apply or validate required persistence migrations
- **THEN** Workspace Catalog commands SHALL reject with a Workspace Catalog migration failure error
- **AND** the command response MUST NOT expose raw SQL, raw SQLx errors, or raw migration implementation details

#### Scenario: Workspace Catalog data is corrupt or inconsistent

- **WHEN** a persisted Workspace row cannot be decoded, fails domain validation, or disagrees with its indexed SQLite row data
- **THEN** Workspace Catalog commands SHALL reject with a Workspace Catalog corruption or schema mismatch error
- **AND** the command response MUST NOT expose raw `workspace_json`

#### Scenario: Workspace UUID already exists

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL continue to reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record

### Requirement: Workspace Setup command errors guide frontend recovery

Workspace Setup command errors SHALL give React enough UI-safe information to present targeted recovery actions.

#### Scenario: Workspace Setup command fails

- **WHEN** any Workspace Setup read or mutation command fails
- **THEN** React SHALL be able to distinguish whether the user should retry, refresh provider setup, reload catalogs, refresh Workspace Catalog, reselect placement data, change a request field, or recover local storage
- **AND** React MUST NOT infer recovery behavior by parsing command error messages

