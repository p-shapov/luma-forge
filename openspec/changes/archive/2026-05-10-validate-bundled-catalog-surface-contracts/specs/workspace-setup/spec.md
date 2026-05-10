## MODIFIED Requirements

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

## ADDED Requirements

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
