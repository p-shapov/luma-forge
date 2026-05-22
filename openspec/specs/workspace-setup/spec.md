# Workspace Setup Specification

## Purpose
Define Native-owned Workspace Setup from bundled workflow and runtime catalog data.

## Requirements
### Requirement: Read bundled Workflow Catalog
The Native Layer SHALL expose a command that returns the bundled Workflow Catalog available in the current application build. Every Workflow Preset declared by the bundled Workflow Catalog SHALL satisfy the Native Layer's offline surface validation before any catalog data is exposed or accepted.

#### Scenario: Workflow Catalog is available
- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Catalog containing selectable Workflow Presets
- **AND** every returned Workflow Preset SHALL include a required runtime contract reference instead of direct Endpoint Worker image refs or ComfyUI Git source fields
- **AND** every returned Workflow Preset SHALL include a required provisioner contract reference instead of direct Provisioner Worker image refs, volume mount paths, or unversioned provisioning defaults
- **AND** every returned model asset SHALL include public Hugging Face download metadata with repository id, file path, revision, and explicit ComfyUI-relative install path
- **AND** every returned model asset MUST NOT include extra app-owned asset metadata
- **AND** the Workflow Catalog MUST NOT expose runtime-provisioned runtime extension declarations
- **AND** the response MUST NOT read or mutate the Workspace Catalog

#### Scenario: Workflow Catalog is unavailable or invalid
- **WHEN** the Client requests the Workflow Catalog and the bundled catalog is unavailable, unreadable, empty, internally inconsistent, or contains unsafe or malformed Workflow Preset surface data
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT mutate the Workspace Catalog

### Requirement: Create Workspace with resolved provisioner image
Workspace Setup SHALL resolve and snapshot the selected Workflow Preset's provisioner contract when creating a Workspace.

#### Scenario: Workspace is created from valid catalog data
- **WHEN** the Client creates a Workspace with a Placement Plan that references a valid Workflow Preset
- **THEN** the Native Layer SHALL resolve the Workflow Preset's runtime contract through the bundled Runtime Catalog
- **AND** the Native Layer SHALL resolve the Workflow Preset's provisioner contract through the bundled Provisioner Catalog
- **AND** the created Workspace SHALL include both `resolved_runtime_image` and `resolved_provisioner_image`
- **AND** `resolved_provisioner_image` SHALL include the provisioner contract id, provisioner contract version, immutable Provisioner Worker image ref, and workspace volume mount path

#### Scenario: Provisioner Catalog data is unavailable during Workspace creation
- **WHEN** the Client creates a Workspace and the selected Workflow Preset's provisioner contract cannot be resolved
- **THEN** the Native Layer SHALL reject Workspace creation with a UI-safe catalog error before persisting the Workspace
- **AND** the Native Layer MUST NOT create a Workspace using a hard-coded Provisioner Worker image ref or hard-coded volume mount path
