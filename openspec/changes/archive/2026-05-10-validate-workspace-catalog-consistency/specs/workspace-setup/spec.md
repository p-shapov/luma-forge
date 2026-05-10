## MODIFIED Requirements

### Requirement: Read Workspace Catalog

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL verify that each returned persisted Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, migration, read, decoding, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate provider setup, bundled catalog compatibility, profile compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL persist one Workspace Catalog entry in SQLite with lifecycle state `draft`
- **AND** the Native Layer SHALL re-read the persisted Workspace record from SQLite
- **AND** the Native Layer SHALL verify that the re-read Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL return the re-read Workspace record as authoritative

#### Scenario: Duplicate Workspace UUID

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL reject the request with `workspace_already_exists`
- **AND** the Native Layer MUST NOT mutate the existing Workspace record
- **AND** the Native Layer MUST NOT create a second Workspace record for the same Workspace UUID

#### Scenario: Provider setup is missing during Workspace creation

- **WHEN** the Client submits a Workspace creation request and the required local Provider API Key is missing
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`
- **AND** the Native Layer MUST NOT persist a Workspace record

#### Scenario: Workspace Catalog write fails

- **WHEN** the Client submits a valid Workspace creation request but SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success
