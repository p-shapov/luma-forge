## MODIFIED Requirements

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

## ADDED Requirements

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
