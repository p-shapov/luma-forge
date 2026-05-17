# runtime-catalog Specification

## Purpose
TBD - created by archiving change bake-comfyui-runtime-into-provisioner-image. Update Purpose after archive.
## Requirements
### Requirement: Provide bundled Runtime Catalog
The Native Layer SHALL read a bundled Runtime Catalog that maps runtime contract ids and versions to the immutable worker image refs used by new Workspaces.

#### Scenario: Runtime Catalog is available
- **WHEN** Workspace Setup needs runtime contract data
- **THEN** the Native Layer SHALL read the bundled Runtime Catalog from the current application build
- **AND** each runtime contract entry SHALL include a stable runtime contract id and one or more revision entries
- **AND** each revision entry SHALL include a contract version, immutable Provisioner Worker image ref, and immutable Endpoint Worker image ref
- **AND** each runtime contract entry MUST NOT include display name, implementation revision, default implementation revision, runtime metadata, image metadata, runtime manifest compatibility metadata, workspace overlay policy metadata, release compatibility metadata, provider secrets, registry credentials, or worker bearer tokens

#### Scenario: Runtime Catalog is unavailable
- **WHEN** the bundled Runtime Catalog cannot be read by the current application build
- **THEN** dependent Workspace Setup operations SHALL fail with a UI-safe catalog error
- **AND** the Native Layer MUST NOT create or update Workspace records from unavailable runtime catalog data

### Requirement: Workflow Presets reference runtime contracts
Workflow Presets SHALL require an exact runtime contract id and version from the bundled Runtime Catalog instead of selecting a ComfyUI Git revision or implementation revision for provisioning-time installation.

#### Scenario: Workflow Preset runtime contract reference exists
- **WHEN** a bundled Workflow Preset declares a `runtime_contract` object with an `id` and `version` pair that exists in the bundled Runtime Catalog
- **THEN** the Native Layer SHALL treat the runtime requirement as resolvable catalog data when all other Workflow Preset rules pass

#### Scenario: Workflow Preset runtime contract reference is missing
- **WHEN** a bundled Workflow Preset declares a runtime contract id/version pair that cannot be resolved through the bundled Runtime Catalog
- **THEN** the Native Layer SHALL reject Workflow Catalog reads and Workspace creation before persisting a Workspace

### Requirement: Persist resolved runtime contract implementation snapshots
Workspace Setup SHALL persist the resolved runtime image snapshot selected for a Draft Workspace.

#### Scenario: Draft Workspace is created
- **WHEN** Workspace Setup creates a Draft Workspace from a Workflow Preset with a resolvable runtime contract id/version pair
- **THEN** it SHALL resolve that id/version pair through the bundled Runtime Catalog
- **AND** it SHALL persist the resolved runtime snapshot with the Workspace
- **AND** the snapshot SHALL include only the runtime contract id, runtime contract version, immutable Provisioner Worker image ref, and immutable Endpoint Worker image ref

#### Scenario: Runtime Catalog changes later
- **WHEN** a later application build changes image refs for a runtime contract id/version pair or adds newer runtime contract versions
- **THEN** existing Workspace records SHALL remain pinned to their persisted resolved runtime image snapshots
- **AND** existing Workspace records MUST NOT silently retarget to newer worker image refs

### Requirement: Runtime contract versions and implementations are immutable
Published runtime contract id/version pairs SHALL retain stable meaning for Workflow Presets and persisted Workspace snapshots.

#### Scenario: Runtime compatibility changes
- **WHEN** a newer ComfyUI, Python, PyTorch/CUDA dependency set, base runtime requirement set, workspace overlay behavior, runtime manifest shape, or image runtime layout changes the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL use a new runtime contract version under the relevant contract id for future Workspaces
- **AND** it MUST NOT mutate an existing runtime contract id/version pair in a way that changes the meaning of persisted Workspace snapshots

#### Scenario: Worker image pair changes during development
- **WHEN** a new Provisioner Worker or Endpoint Worker image pair is published for the current runtime compatibility surface
- **THEN** the Runtime Catalog SHALL point the relevant runtime contract id/version pair at the new immutable image refs for future Workspaces
- **AND** existing Workspace records MUST remain pinned to their persisted image refs

#### Scenario: Runtime implementation is rolled back
- **WHEN** developers need to roll back a runtime image pair during development
- **THEN** they SHALL update the Runtime Catalog contract id/version entry to point future Workspaces at the selected immutable image refs
- **AND** they MUST NOT repoint existing persisted Workspace runtime snapshots

### Requirement: Runtime Catalog update PRs contain only catalog changes
The runtime recipe release workflow SHALL ensure automated Runtime Catalog update PRs contain only the intended bundled Runtime Catalog file.

#### Scenario: Catalog update PR is opened
- **WHEN** the runtime recipe release workflow generates a Runtime Catalog update PR
- **THEN** the PR SHALL include changes to `bundled/runtime-catalog.json`
- **AND** the PR MUST NOT include generated Python packaging artifacts, worker build outputs, or other non-catalog files

#### Scenario: Worker validation creates generated files
- **WHEN** worker validation, package installation, tests, or image build preparation creates generated files in the repository checkout
- **THEN** the workflow SHALL prevent those generated files from being included in the Runtime Catalog update PR

#### Scenario: Unexpected tracked changes remain before PR creation
- **WHEN** the runtime recipe release workflow is ready to open the Runtime Catalog update PR
- **AND** the repository has changed tracked or untracked paths other than `bundled/runtime-catalog.json`
- **THEN** the workflow SHALL fail before creating or updating the PR
- **AND** the workflow SHALL report the unexpected changed paths for diagnosis
