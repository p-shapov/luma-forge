## MODIFIED Requirements

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

### Requirement: Validate Placement Plan against bundled catalogs
The Native Layer SHALL treat the bundled Workflow Catalog as authoritative when validating the provider-discriminated Placement Plan submitted by the Client.

#### Scenario: Submitted Workflow Preset matches bundled definition
- **WHEN** the Client submits a Placement Plan whose selected Workflow Preset matches the bundled definition by id and content
- **THEN** the Native Layer SHALL accept that Workflow Preset for Workspace creation validation
- **AND** the Native Layer SHALL persist the selected Workflow Preset as a creation-time Workspace snapshot
- **AND** the selected Workflow Preset SHALL include a required runtime contract reference that resolves through the bundled Runtime Catalog

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
- **AND** the selected GPU MUST NOT change the base runtime dependency set declared by the resolved runtime contract or the Custom Node dependency set declared by the selected Workflow Preset

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

## ADDED Requirements

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
