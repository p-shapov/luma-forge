## MODIFIED Requirements

### Requirement: Runtime contract versions and implementations are immutable
Published runtime contract id/version pairs and their implementation revisions SHALL retain stable meaning.

#### Scenario: Runtime compatibility changes
- **WHEN** a newer ComfyUI, Python, PyTorch/CUDA dependency set, base runtime requirement set, or runtime manifest contract changes the base runtime compatibility surface
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

#### Scenario: Runtime recipe release reuses an existing contract version
- **WHEN** the runtime recipe release workflow prepares to append an implementation revision under an existing runtime contract id/version
- **THEN** it SHALL verify that the selected recipe's Python version, platform, ComfyUI revision, PyTorch index URL, PyTorch package list, base requirements, and runtime manifest compatibility metadata match the existing catalog contract
- **AND** it SHALL reject the catalog update before image publication when any compatibility field differs
