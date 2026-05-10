## ADDED Requirements

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

## MODIFIED Requirements

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
