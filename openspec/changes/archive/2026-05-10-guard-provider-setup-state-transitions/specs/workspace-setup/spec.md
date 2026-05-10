## ADDED Requirements

### Requirement: Preserve provider setup prerequisite during Workspace creation

The Native Layer SHALL prevent provider setup deletion from interleaving with Workspace creation for the same GPU Cloud Provider between provider setup validation and Draft Workspace persistence.

#### Scenario: Workspace creation and provider setup deletion are serialized

- **WHEN** Workspace creation for `runpod` starts while provider setup deletion for `runpod` is evaluating or mutating local setup state
- **THEN** Workspace creation SHALL evaluate provider setup completeness only after the delete operation has finished
- **AND** Workspace creation SHALL reject with `provider_setup_incomplete` if the required local Provider API Key is missing
- **AND** Workspace creation MUST NOT persist a Workspace record when provider setup is incomplete

#### Scenario: Provider setup deletion waits for Workspace creation persistence

- **WHEN** provider setup deletion for `runpod` starts while Workspace creation for `runpod` is validating provider setup and persisting a Draft Workspace
- **THEN** provider setup deletion SHALL wait until Workspace creation has either persisted and re-read the Workspace record or failed
- **AND** Workspace creation SHALL persist only after confirming provider setup is complete inside the serialized operation

#### Scenario: Workspace duplicate handling remains database-owned

- **WHEN** two Workspace creation requests use the same Workspace UUID concurrently
- **THEN** the Native Layer SHALL rely on the Workspace Catalog uniqueness boundary to persist at most one Workspace record for that UUID
- **AND** the losing request SHALL reject with `workspace_already_exists`
- **AND** provider setup serialization MUST NOT replace SQLite uniqueness enforcement
