# Runtime Catalog Specification

## Purpose
Define the bundled Runtime Catalog as the runtime contract versioning and endpoint image selection source.

## Requirements
### Requirement: Provide bundled Runtime Catalog
The Native Layer SHALL read a bundled Runtime Catalog that maps runtime contract ids and versions to the immutable Endpoint Worker image refs used by new Workspaces.

#### Scenario: Runtime Catalog is available
- **WHEN** Workspace Setup needs runtime contract data
- **THEN** the Native Layer SHALL read the bundled Runtime Catalog from the current application build
- **AND** each runtime contract entry SHALL include a stable runtime contract id and one or more revision entries
- **AND** each revision entry SHALL include a contract version and immutable Endpoint Worker image ref
- **AND** each runtime contract entry MUST NOT include Provisioner Worker image refs, display name, implementation revision, default implementation revision, runtime metadata, image metadata, runtime manifest compatibility metadata, workspace overlay policy metadata, release compatibility metadata, provider secrets, registry credentials, or worker bearer tokens

#### Scenario: Runtime Catalog is unavailable
- **WHEN** the bundled Runtime Catalog cannot be read by the current application build
- **THEN** dependent Workspace Setup operations SHALL fail with a UI-safe catalog error
- **AND** the Native Layer MUST NOT create or update Workspace records from unavailable runtime catalog data

### Requirement: Workflow Presets reference runtime contracts
Workflow Presets SHALL require an exact runtime contract id and version from the bundled Runtime Catalog instead of selecting a direct Endpoint Worker image ref, ComfyUI Git revision, or implementation revision for provisioning-time installation.

#### Scenario: Workflow Preset runtime contract reference exists
- **WHEN** a bundled Workflow Preset declares a `runtime_contract` object with an `id` and `version` pair that exists in the bundled Runtime Catalog
- **THEN** the Native Layer SHALL treat the runtime requirement as resolvable catalog data when all other Workflow Preset rules pass

#### Scenario: Workflow Preset runtime contract reference is missing
- **WHEN** a bundled Workflow Preset declares a runtime contract id/version pair that cannot be resolved through the bundled Runtime Catalog
- **THEN** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation before persisting a Workspace

### Requirement: Runtime contract versions and implementations are immutable
Published runtime contract id/version pairs SHALL retain stable meaning for Workflow Presets and persisted Workspace snapshots.

#### Scenario: Runtime compatibility changes
- **WHEN** a newer ComfyUI, Python, PyTorch/CUDA dependency set, workflow runtime implementation, endpoint image runtime layout, or endpoint workflow contract changes the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL use a new runtime contract version under the relevant contract id for future Workspaces
- **AND** it MUST NOT mutate an existing runtime contract id/version pair in a way that changes the meaning of persisted Workspace snapshots

#### Scenario: Endpoint image changes during development
- **WHEN** a new Endpoint Worker image is published for the current runtime compatibility surface
- **THEN** the Runtime Catalog SHALL point the relevant runtime contract id/version pair at the new immutable endpoint image ref for future Workspaces
- **AND** existing Workspace records MUST remain pinned to their persisted image refs

#### Scenario: Runtime implementation is rolled back
- **WHEN** developers need to roll back a runtime endpoint image during development
- **THEN** they SHALL update the Runtime Catalog contract id/version entry to point future Workspaces at the selected immutable endpoint image ref
- **AND** they MUST NOT repoint existing persisted Workspace runtime snapshots
