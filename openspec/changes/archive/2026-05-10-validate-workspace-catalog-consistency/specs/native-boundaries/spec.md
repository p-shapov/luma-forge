## MODIFIED Requirements

### Requirement: Workspace persistence stores provider identifiers from workspace data

Workspace catalog persistence SHALL derive persisted provider identifiers from the workspace record being stored, and SHALL reject persisted Workspace rows whose indexed data is inconsistent with the serialized Workspace payload.

#### Scenario: Workspace is inserted

- **WHEN** the Workspace Catalog inserts a Workspace record
- **THEN** the stored `gpu_cloud_provider_id` column SHALL be derived from `workspace.gpu_cloud_provider_id`
- **AND** persistence MUST NOT hardcode the v1 provider identifier

#### Scenario: Workspace is re-read after insert

- **WHEN** the Workspace Catalog re-reads a persisted Workspace record
- **THEN** the returned Workspace SHALL match the serialized Workspace payload
- **AND** the indexed provider identifier SHALL remain consistent with that payload

#### Scenario: Workspace row data is inconsistent with payload

- **WHEN** the Workspace Catalog reads a persisted Workspace row whose indexed `id`, `name`, `gpu_cloud_provider_id`, `lifecycle_state`, or `workflow_preset_id` value disagrees with the serialized Workspace payload
- **THEN** the Workspace Catalog SHALL reject the read as unavailable
- **AND** the inconsistent Workspace MUST NOT be returned as authoritative durable state
