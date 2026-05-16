## ADDED Requirements

### Requirement: Provide bundled Runtime Catalog
The Native Layer SHALL read a bundled Runtime Catalog that defines exact runtime contract versions and their verified worker image implementations.

#### Scenario: Runtime Catalog is available
- **WHEN** the Client or Workspace Setup needs runtime contract data
- **THEN** the Native Layer SHALL read the bundled Runtime Catalog from the current application build
- **AND** each runtime contract SHALL include a stable id, exact version, display name, runtime metadata, a non-empty list of immutable implementation revisions, and a default implementation revision
- **AND** each implementation revision SHALL include a stable revision identifier, immutable provisioner image ref, immutable endpoint image ref, and verified image metadata
- **AND** the Runtime Catalog response MUST NOT include provider secrets, registry credentials, or worker bearer tokens

#### Scenario: Runtime Catalog is invalid
- **WHEN** the bundled Runtime Catalog is missing, unreadable, empty, internally inconsistent, references mutable image tags, contains malformed runtime contract data, contains duplicate implementation revisions, or points a default implementation revision at a missing implementation
- **THEN** the Native Layer SHALL reject Runtime Catalog reads and dependent Workspace Setup operations with a UI-safe catalog error
- **AND** the Native Layer MUST NOT create or update Workspace records from invalid runtime contract data

### Requirement: Workflow Presets reference runtime contracts
Workflow Presets SHALL require an exact runtime contract id and version from the bundled Runtime Catalog instead of selecting a ComfyUI Git revision for provisioning-time installation.

#### Scenario: Workflow Preset runtime contract exists
- **WHEN** a bundled Workflow Preset declares a `required_runtime_contract` whose id and version exist in the bundled Runtime Catalog
- **THEN** the Native Layer SHALL treat the runtime requirement as valid catalog data when all other Workflow Preset validation passes

#### Scenario: Workflow Preset runtime contract is missing
- **WHEN** a bundled Workflow Preset declares a missing, blank, malformed, or unknown runtime contract reference
- **THEN** the Native Layer SHALL treat the bundled catalog set as invalid
- **AND** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation before persisting a Workspace

### Requirement: Persist resolved runtime contract implementation snapshots
Workspace Setup SHALL persist the resolved runtime contract implementation snapshot selected for a Draft Workspace.

#### Scenario: Draft Workspace is created
- **WHEN** Workspace Setup creates a Draft Workspace from a Workflow Preset with a valid runtime contract reference
- **THEN** it SHALL resolve the runtime contract through the bundled Runtime Catalog
- **AND** it SHALL select the contract's default implementation revision
- **AND** it SHALL persist the resolved runtime contract implementation snapshot with the Workspace
- **AND** the snapshot SHALL include the runtime contract id, version, selected implementation revision, immutable provisioner image ref, immutable endpoint image ref, runtime metadata needed by provisioning and endpoint validation, and verified image metadata

#### Scenario: Runtime Catalog changes later
- **WHEN** a later application build adds newer runtime contract versions or newer implementation revisions
- **THEN** existing Workspace records SHALL remain pinned to their persisted resolved runtime contract implementation snapshots
- **AND** existing Workspace records MUST NOT silently retarget to newer implementation revisions or worker image refs

### Requirement: Runtime contract versions and implementations are immutable
Published runtime contract id/version pairs and their implementation revisions SHALL retain stable meaning.

#### Scenario: Runtime compatibility changes
- **WHEN** a newer ComfyUI, Python, PyTorch/CUDA dependency set, or runtime manifest contract changes the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL add a new runtime contract version
- **AND** it MUST NOT mutate an existing runtime contract version in a way that changes the meaning of persisted Workspace snapshots

#### Scenario: Worker implementation changes without runtime compatibility change
- **WHEN** a new Provisioner Worker or Endpoint Worker image pair is published for an existing runtime contract id/version without changing the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL append a new immutable implementation revision under that runtime contract
- **AND** it MAY set that implementation revision as the default for future Workspaces
- **AND** it MUST NOT mutate the image refs or verified metadata of an existing implementation revision

#### Scenario: Runtime implementation is rolled back
- **WHEN** operators need to roll back a runtime implementation
- **THEN** they SHALL select a previously published immutable implementation revision from a Runtime Catalog entry as the default for future Workspaces or add a new reviewed runtime contract version
- **AND** they MUST NOT repoint an existing persisted runtime contract implementation snapshot by mutating its implementation revision or image refs in place
