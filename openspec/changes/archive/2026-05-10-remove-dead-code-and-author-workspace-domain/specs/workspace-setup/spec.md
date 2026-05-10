## MODIFIED Requirements

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model before the record is mapped to the serializable Workspace Catalog shape.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled catalog compatibility, profile compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL construct the Draft Workspace through the domain Workspace model
- **AND** the domain-authored Workspace SHALL have lifecycle state `draft`
- **AND** the domain-authored Workspace SHALL have empty Persistent Storage Volume, active Provisioning Pod, Serverless Endpoint, and last Provisioning Pod snapshots
- **AND** the Native Layer SHALL map the domain-authored Workspace to the serializable Workspace Catalog record
- **AND** the Native Layer SHALL persist one Workspace Catalog entry in SQLite with lifecycle state `draft`
- **AND** the Native Layer SHALL re-read the persisted Workspace record from SQLite
- **AND** the Native Layer SHALL verify that the re-read Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL return the re-read Workspace record as authoritative
- **AND** Workspace creation MUST NOT require a live Provider identity check

#### Scenario: Duplicate Workspace UUID

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record
- **AND** the Native Layer MUST NOT create a second Workspace record for the same Workspace UUID

#### Scenario: Provider API Key is missing during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Provider API Key is unreadable during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key cannot be parsed as a secret value
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT call the Provider to validate identity
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace Catalog write fails

- **WHEN** the Client submits a valid Workspace creation request but SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success
