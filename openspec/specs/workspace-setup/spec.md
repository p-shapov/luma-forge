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
- **AND** every returned Workflow Preset SHALL include a `requires_hugging_face_api_key` flag
- **AND** every returned model asset SHALL include Hugging Face download metadata with repository id, file path, revision, and explicit ComfyUI-relative install path
- **AND** every returned model asset MUST NOT include raw Hugging Face API keys, credential-bearing URLs, provider secrets, worker bearer tokens, or runtime-provisioned asset metadata
- **AND** the Workflow Catalog MUST NOT expose runtime-provisioned runtime extension declarations

#### Scenario: Workflow Catalog is unavailable or invalid
- **WHEN** the Client requests the Workflow Catalog and the bundled catalog is unavailable, unreadable, empty, internally inconsistent, or contains unsafe or malformed Workflow Preset surface data
- **THEN** the Native Layer SHALL reject the request with `workflow_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial catalog data

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

### Requirement: Provide bundled HiDream O1 Dev Workflow Preset
The bundled Workflow Catalog SHALL expose `comfyui-hidream-o1-dev` as the supported text-to-image Workflow Preset instead of the prior SDXL preset.

#### Scenario: HiDream O1 Dev preset is available
- **WHEN** the Client requests the Workflow Catalog
- **THEN** the Native Layer SHALL return a Workflow Preset with id `comfyui-hidream-o1-dev`
- **AND** the preset SHALL have workflow execution type `t2i`
- **AND** the preset SHALL reference runtime contract id `comfyui-hidream-o1-dev`
- **AND** the preset SHALL reference an exact runtime contract version that exists in the bundled Runtime Catalog
- **AND** the preset SHALL reference an exact provisioner contract version that exists in the bundled Provisioner Catalog

#### Scenario: HiDream O1 Dev model assets are declared
- **WHEN** the Native Layer validates the bundled Workflow Catalog
- **THEN** the `comfyui-hidream-o1-dev` Workflow Preset SHALL declare the HiDream O1 Dev checkpoint asset from `Comfy-Org/HiDream-O1-Image`
- **AND** it SHALL install that checkpoint under `models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors`
- **AND** it SHALL declare the Gemma text encoder asset from `Comfy-Org/gemma-4`
- **AND** it SHALL install that text encoder under `models/text_encoders/gemma4_e4b_it_fp8_scaled.safetensors`
- **AND** every declared asset SHALL include a non-empty immutable Hugging Face revision

### Requirement: Declare Hugging Face workflow authentication requirements
Bundled Workflow Presets SHALL declare whether the workflow requires a configured Hugging Face API key before provisioning can download its model assets.

#### Scenario: Workflow requiring authenticated model assets is declared
- **WHEN** a bundled Workflow Preset includes a Hugging Face model asset that requires authenticated access
- **THEN** the Workflow Preset SHALL set `requires_hugging_face_api_key` to `true`
- **AND** the Workflow Catalog response MUST NOT include raw Hugging Face API keys or credential-bearing download URLs

#### Scenario: Public model asset is declared
- **WHEN** a bundled Workflow Preset includes a Hugging Face model asset that can be downloaded without authentication
- **THEN** the Workflow Preset SHALL set `requires_hugging_face_api_key` to `false`
- **AND** Workspace Setup MUST NOT require Hugging Face setup solely because public Hugging Face assets exist

