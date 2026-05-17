## MODIFIED Requirements

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
