## ADDED Requirements

### Requirement: Prepared runtime dependency records are workspace-resolved
The prepared runtime manifest SHALL advertise base dependency record paths that resolve under the mounted workspace and correspond to files published during runtime materialization.

#### Scenario: Manifest dependency records resolve under workspace
- **WHEN** the Provisioner Worker writes the prepared runtime manifest
- **THEN** every `base_dependency_record_paths` entry SHALL resolve under the mounted workspace path
- **AND** each entry SHALL identify a file that exists before terminal success is reported

#### Scenario: Relative catalog record paths are converted before manifest write
- **WHEN** the resolved runtime metadata contains relative base dependency record paths from the Runtime Catalog
- **THEN** the Provisioner Worker SHALL convert them to workspace-resolved manifest paths
- **AND** the Endpoint Worker MUST NOT resolve those manifest paths relative to its process working directory

### Requirement: Runtime archive format is extractable by provisioner
The runtime archive format declared by the Runtime Catalog implementation metadata SHALL be extractable by the Provisioner Worker runtime shipped in the corresponding provisioner image.

#### Scenario: Runtime archive is materialized
- **WHEN** the Provisioner Worker receives a start request for its matching resolved runtime implementation
- **THEN** it SHALL extract the configured runtime archive using a supported decompression path
- **AND** it SHALL publish the staged ComfyUI tree, virtual environment, and base runtime dependency records before writing the terminal runtime manifest

#### Scenario: Unsupported archive compression fails safely
- **WHEN** the configured runtime archive exists but cannot be decompressed or opened by the Provisioner Worker
- **THEN** the Provisioner Worker SHALL fail the active preparation job
- **AND** it MUST NOT write a terminal success runtime manifest
