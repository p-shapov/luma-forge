# Provisioner Catalog Specification

## Purpose
Define bundled provisioner contract metadata, immutable Provisioner Worker image selection, and resolved provisioner snapshots.
## Requirements
### Requirement: Provide bundled Provisioner Catalog
The Native Layer SHALL read a bundled Provisioner Catalog that maps provisioner contract ids and versions to immutable Provisioner Worker image refs used by new Workspaces.

#### Scenario: Provisioner Catalog is available
- **WHEN** Workspace Setup needs provisioner contract data
- **THEN** the Native Layer SHALL read the bundled Provisioner Catalog from the current application build
- **AND** each provisioner contract entry SHALL include a stable provisioner contract id and one or more revision entries
- **AND** each revision entry SHALL include a contract version and immutable Provisioner Worker image ref
- **AND** each provisioner contract entry MUST NOT include Endpoint Worker image refs, provider secrets, registry credentials, user secrets, or worker bearer tokens

#### Scenario: Provisioner Catalog is unavailable
- **WHEN** the bundled Provisioner Catalog cannot be read by the current application build
- **THEN** dependent Workspace Setup operations SHALL fail with a UI-safe catalog error
- **AND** the Native Layer MUST NOT create or update Workspace records from unavailable provisioner catalog data

### Requirement: Provisioner contract versions are immutable
Published provisioner contract id/version pairs SHALL retain stable meaning for Workflow Presets and persisted Workspace snapshots.

#### Scenario: Provisioner image is published
- **WHEN** a new Provisioner Worker image is published for the provisioner contract
- **THEN** the release tooling SHALL promote the published image by proposing a new Provisioner Catalog revision under the relevant contract id for future Workspaces
- **AND** the new revision SHALL point at the published immutable Provisioner Worker image ref
- **AND** the new revision SHALL retain the provisioner metadata needed to resolve Workspaces
- **AND** it MUST NOT mutate any existing provisioner contract id/version pair in a way that changes the meaning of persisted Workspace snapshots

### Requirement: Resolved provisioner image snapshots are persisted
Workspace records SHALL include a resolved provisioner image snapshot derived from the selected Workflow Preset's provisioner contract reference and the bundled Provisioner Catalog.

#### Scenario: Provisioner contract resolves
- **WHEN** Workspace Setup creates a Workspace from a Workflow Preset whose provisioner contract reference exists in the bundled Provisioner Catalog
- **THEN** the Native Layer SHALL persist a resolved provisioner image snapshot on the Workspace
- **AND** the snapshot SHALL include the provisioner contract id, provisioner contract version, and immutable Provisioner Worker image ref
- **AND** Workspace Provisioning SHALL treat the persisted snapshot as authoritative for provisioning image decisions

#### Scenario: Provisioner contract cannot resolve
- **WHEN** Workspace Setup creates a Workspace from a Workflow Preset whose provisioner contract reference cannot be resolved through the bundled Provisioner Catalog
- **THEN** the Native Layer SHALL reject Workspace creation before persisting a Workspace
- **AND** Workspace creation SHALL use resolved bundled Provisioner Catalog metadata for provisioner image decisions
