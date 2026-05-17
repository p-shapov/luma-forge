# runtime-catalog Specification

## Purpose
TBD - created by archiving change bake-comfyui-runtime-into-provisioner-image. Update Purpose after archive.
## Requirements
### Requirement: Provide bundled Runtime Catalog
The Native Layer SHALL read a bundled Runtime Catalog that defines exact runtime contract versions and their verified worker image implementations, including image-baked runtime metadata and workspace overlay policy.

#### Scenario: Runtime Catalog is available
- **WHEN** the Client or Workspace Setup needs runtime contract data
- **THEN** the Native Layer SHALL read the bundled Runtime Catalog from the current application build
- **AND** each runtime contract SHALL include a stable id, exact version, display name, runtime metadata, a non-empty list of immutable implementation revisions, and a default implementation revision
- **AND** each implementation revision SHALL include a stable revision identifier, immutable provisioner image ref, immutable endpoint image ref, verified image metadata, image runtime root metadata, image Python interpreter metadata, image ComfyUI root metadata, base dependency record metadata, runtime manifest compatibility metadata, and workspace overlay policy metadata
- **AND** the Runtime Catalog response MUST NOT include provider secrets, registry credentials, or worker bearer tokens

#### Scenario: Runtime Catalog is invalid
- **WHEN** the bundled Runtime Catalog is missing, unreadable, empty, internally inconsistent, references mutable image tags, contains malformed runtime contract data, contains duplicate implementation revisions, points a default implementation revision at a missing implementation, omits required image runtime metadata, or omits required overlay policy metadata
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
- **AND** the snapshot SHALL include the runtime contract id, version, selected implementation revision, immutable provisioner image ref, immutable endpoint image ref, runtime metadata needed by provisioning and endpoint validation, image-baked runtime root metadata, image Python interpreter metadata, image ComfyUI root metadata, base dependency record metadata, runtime manifest compatibility metadata, workspace overlay policy metadata, and verified image metadata

#### Scenario: Runtime Catalog changes later
- **WHEN** a later application build adds newer runtime contract versions or newer implementation revisions
- **THEN** existing Workspace records SHALL remain pinned to their persisted resolved runtime contract implementation snapshots
- **AND** existing Workspace records MUST NOT silently retarget to newer implementation revisions or worker image refs

### Requirement: Runtime contract versions and implementations are immutable
Published runtime contract id/version pairs and their implementation revisions SHALL retain stable meaning.

#### Scenario: Runtime compatibility changes
- **WHEN** a newer ComfyUI, Python, PyTorch/CUDA dependency set, base runtime requirement set, workspace overlay policy, runtime manifest contract, or image runtime layout changes the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL add a new runtime contract version
- **AND** it MUST NOT mutate an existing runtime contract version in a way that changes the meaning of persisted Workspace snapshots

#### Scenario: Worker implementation changes without runtime compatibility change
- **WHEN** a new Provisioner Worker or Endpoint Worker image pair is published for an existing runtime contract id/version without changing the base runtime compatibility surface
- **THEN** the Runtime Catalog SHALL append a new immutable implementation revision under that runtime contract
- **AND** it MAY set that implementation revision as the default for future Workspaces
- **AND** it MUST NOT mutate the image refs or verified metadata of an existing implementation revision

#### Scenario: Runtime recipe release auto-selects an implementation revision
- **WHEN** the runtime recipe release workflow prepares a new implementation revision for an existing runtime contract id/version without an explicit implementation revision
- **THEN** it SHALL derive the implementation revision from the current UTC date and the next unused sequence for that date under the selected runtime contract
- **AND** it SHALL format the derived implementation revision as `YYYY.MM.DD-NNN`
- **AND** it SHALL validate that the derived implementation revision is not already present under the selected runtime contract before worker package validation, Docker image builds, registry publication, or Runtime Catalog PR creation

#### Scenario: Runtime implementation is rolled back
- **WHEN** operators need to roll back a runtime implementation
- **THEN** they SHALL select a previously published immutable implementation revision from a Runtime Catalog entry as the default for future Workspaces or add a new reviewed runtime contract version
- **AND** they MUST NOT repoint an existing persisted runtime contract implementation snapshot by mutating its implementation revision or image refs in place

#### Scenario: Runtime recipe release reuses an existing contract version
- **WHEN** the runtime recipe release workflow prepares to append an implementation revision under an existing runtime contract id/version
- **THEN** it SHALL verify that the selected recipe's Python version, platform, ComfyUI revision, PyTorch index URL, PyTorch package list, base requirements, runtime manifest compatibility metadata, image runtime layout, and workspace overlay policy match the existing catalog contract
- **AND** it SHALL reject the catalog update before image publication when any compatibility field differs

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

