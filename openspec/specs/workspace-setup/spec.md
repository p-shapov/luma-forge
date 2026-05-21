# workspace-setup Specification

## Purpose
Define the native-owned Workspace Setup lifecycle for reading bundled catalogs, validating placement plans, and creating local draft Workspace records without creating provider resources or exposing provider secrets.
## Requirements
### Requirement: Read bundled Workflow Catalog
The Native Layer SHALL expose a command that returns the bundled Workflow Catalog available in the current application build. Every Workflow Preset declared by the bundled Workflow Catalog SHALL satisfy the Native Layer's offline surface validation before any catalog data is exposed or accepted.

#### Scenario: Workflow Catalog is available
- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Catalog containing selectable Workflow Presets
- **AND** every returned Workflow Preset SHALL include a required runtime contract reference instead of ComfyUI Git source fields
- **AND** every returned model asset SHALL include public Hugging Face download metadata with repository id, file path, revision, and explicit ComfyUI-relative install path
- **AND** every returned model asset MUST NOT include extra app-owned asset metadata
- **AND** every returned Custom Node Git source SHALL include an immutable commit revision for worker-prepared checkout
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

### Requirement: Read Workspace Catalog

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog after required Workspace Catalog schema bootstrap and compatibility checks have completed.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL complete required Workspace Catalog schema bootstrap and compatibility checks before decoding rows
- **AND** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL verify that each returned persisted Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, schema bootstrap, compatibility checking, read, decoding, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

### Requirement: Bootstrap Workspace Catalog schema before use

The Native Layer SHALL bootstrap and check the Workspace Catalog SQLite schema before using the Workspace Catalog for reads, duplicate checks, inserts, or post-insert re-reads. Pre-production legacy Workspace Catalog compatibility migrations, downgrade handling, and backfill hooks are not required by the current app contract.

#### Scenario: Fresh Workspace Catalog schema is bootstrapped

- **WHEN** the Native Layer opens a Workspace Catalog that has no recorded persistence version and no existing Workspace Catalog tables
- **THEN** the Native Layer SHALL treat the catalog as a fresh catalog
- **AND** the Native Layer SHALL create the Workspace Catalog metadata table
- **AND** the Native Layer SHALL create the current normalized Workspace Catalog schema
- **AND** the Native Layer SHALL record the current persistence version only after the current schema is created successfully

#### Scenario: Unversioned Workspace Catalog tables already exist

- **WHEN** the Native Layer opens a Workspace Catalog that has no recorded persistence version but already contains Workspace Catalog tables
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_migration_failed`
- **AND** the Native Layer MUST NOT adopt, backfill, migrate, downgrade, read, or write the unversioned Workspace records

#### Scenario: Current Workspace Catalog schema is checked

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version matches the current application persistence version
- **THEN** the Native Layer SHALL validate that the expected current Workspace Catalog schema is present before returning or writing Workspace records
- **AND** the Native Layer SHALL use the existing Workspace records only after the schema check succeeds
- **AND** normal Workspace Catalog read, duplicate check, insert, and row consistency validation rules SHALL still apply

#### Scenario: Workspace Catalog was written by a newer app version

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version is greater than the current application persistence version
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_migration_failed`
- **AND** the Native Layer MUST NOT read, write, migrate, downgrade, backfill, or mutate Workspace records from the newer catalog version

### Requirement: Document pre-production Workspace Catalog reset

The project documentation SHALL describe how developers can manually reset the local pre-production Workspace Catalog database when schema bootstrap or compatibility checks reject stale local state. This documentation SHALL be developer troubleshooting guidance only and MUST NOT define a production user recovery flow.

#### Scenario: Developer resets stale local catalog state

- **WHEN** a developer encounters a local Workspace Catalog startup failure during pre-production development
- **THEN** the README SHALL identify the Workspace Catalog file as `workspace-catalog.sqlite` under the Tauri application data directory
- **AND** the README SHALL include the macOS path pattern `~/Library/Application Support/<app identifier>/workspace-catalog.sqlite`
- **AND** the README SHALL instruct the developer to stop the app before deleting the SQLite file
- **AND** the README SHALL warn that deleting the file removes local Workspace Catalog records
- **AND** the README SHALL warn that deleting the file does not clean up remote provider resources
- **AND** the README MUST NOT present manual deletion as a supported production migration or downgrade path

### Requirement: Persist Workspace Catalog records as normalized SQLite fields

The Native Layer SHALL persist Workspace Catalog records through explicit SQLite fields and related rows for Workspace identity, lifecycle, placement, resolved runtime image, provider resource snapshots, provider provisioning snapshots, environment preparation metadata, and last provisioning failure metadata. A serialized full-Workspace JSON value MUST NOT be the authoritative source for Workspace Catalog reads or writes.

#### Scenario: Draft Workspace is persisted without an authoritative JSON blob

- **WHEN** the Native Layer creates a valid Draft Workspace
- **THEN** the Native Layer SHALL persist the Workspace identity, name, GPU Cloud Provider id, lifecycle state, selected placement fields, selected Workflow Preset identity, and resolved runtime image fields as explicit SQLite data
- **AND** the Native Layer SHALL persist empty provider resource snapshots, provider provisioning snapshot, environment prepared timestamp, and last provisioning failure as explicit absent values
- **AND** the Native Layer MUST NOT require a serialized full-Workspace JSON value to reconstruct the returned Workspace

#### Scenario: Provisioning metadata is persisted as normalized data

- **WHEN** Workspace Provisioning updates a Workspace with provider resource snapshots, provider provisioning snapshots, environment preparation metadata, or last provisioning failure metadata
- **THEN** the Native Layer SHALL persist each updated metadata group as explicit SQLite fields or related rows
- **AND** the Native Layer SHALL update the Workspace lifecycle state and updated timestamp in the same durable operation
- **AND** subsequent Workspace Catalog reads SHALL reconstruct the authoritative Workspace from the normalized SQLite data

#### Scenario: Normalized row data is inconsistent

- **WHEN** the Native Layer reads normalized Workspace Catalog data that cannot be reconstructed into a valid Workspace or whose related rows contradict the Workspace identity, GPU Cloud Provider, lifecycle, placement, runtime, or provider resource invariants
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_schema_mismatch` or `workspace_catalog_corrupt`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

#### Scenario: Fresh normalized catalog schema is initialized

- **WHEN** the Native Layer initializes the Workspace Catalog schema for the current app version
- **THEN** the Native Layer SHALL create normalized SQLite tables and indexes required to persist Workspace Catalog records
- **AND** the Native Layer MUST NOT create a required full-Workspace JSON column for authoritative reads or writes

### Requirement: Preserve SQLite Workspace Catalog behavior across module split

The Native Layer SHALL preserve SQLite Workspace Catalog repository behavior when the implementation is split from a single file into focused SQLite submodules.

#### Scenario: Workspace Catalog repository operations are unchanged

- **WHEN** the Native Layer lists, finds, inserts, or updates Workspace records through `SqliteWorkspaceCatalog`
- **THEN** the SQLite-backed repository SHALL apply the same migrations, validation, persistence, decoding, duplicate detection, and UI-safe error mapping as before the module split
- **AND** callers SHALL continue using the `workspace_catalog::sqlite::SqliteWorkspaceCatalog` module path

#### Scenario: Workspace detail mappings are unchanged

- **WHEN** the SQLite-backed repository persists and re-reads Workspace placement, runtime image, provider resource snapshots, provisioning metadata, or failure metadata
- **THEN** the reconstructed Workspace domain object SHALL remain internally consistent with the persisted SQLite rows
- **AND** the refactor MUST NOT introduce new SQLite tables, migrations, command payload fields, or generated TypeScript contract changes

### Requirement: Read provider placement inventory

The Native Layer SHALL expose a `get_provider_placement_options` command that returns provider placement options for an explicit GPU Cloud Provider after validating local provider setup prerequisites. Placement options SHALL include live Provider Inventory and provider placement capabilities, and provider-specific inventory mapping SHALL only expose datacenters that can satisfy LumaForge's current provisioning prerequisites. Provider-specific placement option behavior SHALL be selected through a service-level Workspace Setup provider capability.

#### Scenario: Provider setup is complete
- **WHEN** the Client requests provider placement options for `runpod` and the local Provider API Key exists
- **THEN** the Native Layer SHALL select the concrete RunPod Workspace Setup provider capability through centralized `GpuCloudProviderId` dispatch
- **AND** the Native Layer SHALL call RunPod using the stored Provider API Key to fetch live Provider Inventory
- **AND** the Native Layer SHALL return available datacenters, GPU options per datacenter, and provider maximum Persistent Storage Volume size when known
- **AND** returned RunPod datacenters SHALL be limited to datacenters whose provider inventory reports support for persistent network storage
- **AND** the Native Layer SHALL return placement capabilities for RunPod endpoint keep-alive with `supported = true`, `default_seconds = 5`, `min_seconds = 5`, and `max_seconds = 3600`
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: RunPod datacenter has GPUs but does not support network storage
- **WHEN** RunPod inventory reports a datacenter with GPU availability and storage support is false or missing
- **THEN** the Native Layer SHALL omit that datacenter from returned provider placement options
- **AND** the Native Layer MUST NOT expose the omitted datacenter as selectable for a new RunPod Placement Plan

#### Scenario: GPU availability is displayed during placement selection
- **WHEN** the Native Layer returns RunPod GPU options with availability scores
- **THEN** the Client SHALL display the availability for each selectable GPU option
- **AND** the Client SHALL surface zero availability as unavailable rather than hiding the GPU solely because the score is zero
- **AND** the Client SHALL disable workspace creation while the selected GPU has zero availability
- **AND** the Client SHALL disable starting provisioning for a selected workspace when loaded placement options show that workspace's selected GPU has zero availability or is absent from currently eligible placement options
- **AND** the Client SHALL continue allowing provisioning sync and cancellation commands for existing workspaces regardless of current GPU availability

#### Scenario: Provider setup is incomplete
- **WHEN** the Client requests provider placement options and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST reject before calling the Provider

#### Scenario: Provider API Key is invalid or revoked
- **WHEN** the Client requests provider placement options and the Provider rejects the stored Provider API Key as unauthorized or forbidden
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT report the failure as retryable
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory request is rate limited
- **WHEN** the Provider inventory request fails because the Provider reports rate limiting
- **THEN** the Native Layer SHALL reject the request with a retryable UI-safe `provider_rate_limited` command error
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog
- **AND** the command error MUST NOT expose Provider API Keys, raw provider request bodies, raw provider response bodies, or provider-specific error codes

#### Scenario: Provider inventory request is temporarily unavailable
- **WHEN** the Provider inventory request fails due to timeout, transport error, or temporarily unavailable Provider API
- **THEN** the Native Layer SHALL reject the request with a retryable UI-safe provider availability error
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog
- **AND** the command error MUST NOT expose Provider API Keys, raw provider request bodies, raw provider response bodies, or provider-specific error codes

#### Scenario: Provider inventory request is rejected
- **WHEN** the Provider rejects the inventory request for a non-authentication request validation reason
- **THEN** the Native Layer SHALL reject the request with a non-retryable UI-safe `provider_request_rejected` command error
- **AND** retryability SHALL be derived from the LumaForge-owned provider error instead of provider-specific error codes or message strings
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider inventory response is invalid
- **WHEN** the Provider inventory request succeeds but the Provider response cannot be parsed or cannot be mapped into valid Provider Inventory
- **THEN** the Native Layer SHALL reject the request with a UI-safe provider inventory or response validation error
- **AND** the Native Layer MUST NOT report the same request as safely retryable solely because parsing failed
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

#### Scenario: Provider does not support endpoint keep-alive
- **WHEN** a future GPU Cloud Provider does not support endpoint keep-alive configuration
- **THEN** its provider placement options SHALL return endpoint keep-alive capability with `supported = false`
- **AND** that provider's Placement Plan variant MUST NOT persist an endpoint keep-alive value

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record after required Workspace Catalog schema bootstrap and compatibility checks have completed.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid RunPod Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled Workflow Preset compatibility, placement structure, and RunPod endpoint keep-alive range before persistence
- **AND** the RunPod Placement Plan SHALL include provider-specific endpoint keep-alive seconds
- **AND** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL complete required Workspace Catalog schema bootstrap and compatibility checks before checking duplicates or writing the new Workspace record
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
- **THEN** the Native Layer SHALL complete required Workspace Catalog schema bootstrap and compatibility checks before evaluating the duplicate Workspace UUID
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

- **WHEN** the Client submits a valid Workspace creation request but Workspace Catalog schema bootstrap, compatibility checking, SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

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
The Native Layer SHALL treat the bundled Workflow Catalog as authoritative when validating the provider-discriminated Placement Plan submitted by the Client.

#### Scenario: Submitted Workflow Preset matches bundled definition
- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset matches the bundled definition by id and content
- **THEN** the Native Layer SHALL accept that Workflow Preset for Workspace creation validation
- **AND** the Native Layer SHALL persist the selected Workflow Preset as a creation-time Workspace snapshot
- **AND** the selected Workflow Preset SHALL include a required runtime contract id/version pair that resolves through the bundled Runtime Catalog

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

#### Scenario: Placement validation does not select runtime dependencies
- **WHEN** the Client submits any RunPod Placement Plan with a selected GPU
- **THEN** the Native Layer SHALL validate placement structure, catalog compatibility, storage size, and endpoint keep-alive range
- **AND** the selected GPU MUST NOT change the worker image refs resolved from the selected Workflow Preset's runtime contract id/version pair or the Custom Node dependency set declared by the selected Workflow Preset

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

Workspace Setup commands MUST NOT return, persist outside secure keyring, log, or include Provider API Keys in generated frontend types, command responses, errors, Workspace records, provider placement options responses, or diagnostics.

#### Scenario: Provider placement options are returned

- **WHEN** the Native Layer returns provider placement options
- **THEN** the response SHALL include only UI-safe provider inventory data
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Workspace is created

- **WHEN** the Native Layer persists or returns a Workspace record
- **THEN** the Workspace record MUST NOT include the Provider API Key
- **AND** the Workspace record SHALL reference only the GPU Cloud Provider id and UI-safe selected configuration snapshots

### Requirement: Validate Workflow Preset source fields
The Native Layer SHALL validate bundled Workflow Preset source fields using offline surface checks before exposing or accepting the Workflow Preset.

#### Scenario: Workflow Preset source fields are valid
- **WHEN** a bundled Workflow Preset declares a runtime contract reference that resolves through the bundled Runtime Catalog and model assets with non-empty Hugging Face repository ids, safe repo-relative file paths, and non-empty revisions
- **THEN** the Native Layer SHALL treat those source fields as valid catalog data
- **AND** the Native Layer SHALL NOT call Docker registries, Git, Hugging Face, or any network service to verify resource existence

#### Scenario: Workflow Preset source fields are invalid
- **WHEN** a bundled Workflow Preset declares a missing, blank, malformed, or unknown runtime contract reference, a malformed Hugging Face repository id, an unsafe model source file path, or a blank model source revision
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

### Requirement: Keep bundled catalog validation offline

Bundled catalog validation SHALL validate local contract shape and safety constraints only.

#### Scenario: External resources are not checked during catalog validation

- **WHEN** the Native Layer validates the bundled Workflow Catalog
- **THEN** the Native Layer MUST NOT call Docker registries, Git repositories, Hugging Face, RunPod, worker HTTP endpoints, or any external service to validate reachability, existence, authenticity, or current availability
- **AND** external availability failures SHALL remain the responsibility of later provisioning or provider operations

### Requirement: Validate worker-prepared Git source revisions
The Native Layer SHALL validate bundled Workflow Preset Git revisions only for Custom Node sources that the Provisioner Worker prepares remotely.

#### Scenario: Worker-prepared Git revisions are immutable
- **WHEN** a bundled Workflow Preset declares Custom Node Git sources with full 40-character lowercase hexadecimal commit revisions
- **THEN** the Native Layer SHALL treat those revisions as valid catalog data
- **AND** Workspace Setup validation MAY accept the Workflow Preset when all other catalog rules pass

#### Scenario: Worker-prepared Git revision is mutable
- **WHEN** a bundled Workflow Preset declares a Custom Node Git source revision as a branch name, tag name, blank value, or non-commit value
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

### Requirement: Provider placement options report provider failure classes precisely

Provider placement options reads SHALL distinguish Provider authorization, provider network/API availability, malformed responses, and invalid mapped inventory.

#### Scenario: Stored Provider API Key is missing

- **WHEN** the Client requests provider placement options and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`

#### Scenario: Stored Provider API Key is unauthorized

- **WHEN** RunPod rejects the stored Provider API Key while fetching provider placement options
- **THEN** the Native Layer SHALL reject the request with a Provider API Key authorization error
- **AND** React SHALL be able to route the user toward Provider Setup recovery

#### Scenario: Provider placement options request cannot reach provider

- **WHEN** RunPod inventory lookup fails due to timeout, DNS, connection failure, request timeout, provider outage, rate limiting, or non-auth provider availability failure
- **THEN** the Native Layer SHALL reject the request with a retryable provider availability error

#### Scenario: Provider placement options response is malformed or invalid

- **WHEN** RunPod inventory lookup returns a response that cannot be parsed, mapped, or validated as provider placement options
- **THEN** the Native Layer SHALL reject the request with a Provider response or inventory invalid error
- **AND** the generated command error MUST NOT include the raw Provider response body

### Requirement: Workspace creation reports request validation failures precisely

Workspace creation SHALL return field-specific UI-safe errors for invalid command request shape before evaluating provider setup, catalogs, placement, or persistence.

#### Scenario: Workspace UUID is invalid

- **WHEN** the Client submits a Workspace creation request whose `workspace_id` is missing or is not a valid UUID
- **THEN** the Native Layer SHALL reject the request with `invalid_workspace_id`
- **AND** the Native Layer MUST NOT read Provider setup, bundled catalogs, provider placement options, or Workspace Catalog persistence

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

### Requirement: Workspace Catalog command errors distinguish safe recovery categories

Workspace Catalog read and write failures SHALL expose safe command-level categories that help React choose retry, recovery, or blocking behavior.

#### Scenario: Local storage path is unavailable

- **WHEN** the Native Layer cannot resolve or create the app data directory or connect to the SQLite catalog file
- **THEN** Workspace Catalog commands SHALL reject with a local storage or Workspace Catalog storage unavailable error

#### Scenario: Workspace Catalog bootstrap or compatibility check fails

- **WHEN** Workspace Catalog initialization cannot bootstrap or validate the required current schema
- **THEN** Workspace Catalog commands SHALL reject with a Workspace Catalog migration failure error
- **AND** the command response MUST NOT expose raw SQL, raw SQLx errors, or raw schema bootstrap implementation details

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

### Requirement: Keep GPU placement separate from dependency selection
Selected GPU validation SHALL remain provider placement validation and MUST NOT select base runtime or Custom Node Python dependencies.

#### Scenario: Selected GPU is accepted for placement
- **WHEN** the selected GPU satisfies provider placement validation for the RunPod Placement Plan
- **THEN** the Native Layer SHALL treat the selected GPU as valid placement input
- **AND** it MUST NOT install a different base runtime or Custom Node Python dependency set for that GPU

#### Scenario: Selected GPU is rejected for placement
- **WHEN** the selected GPU is unavailable, malformed, stale, or invalid for the provider placement request
- **THEN** the Native Layer SHALL reject new placement or provisioning with a UI-safe placement error
- **AND** it MUST NOT attempt to repair compatibility by changing base runtime or Custom Node Python dependencies
