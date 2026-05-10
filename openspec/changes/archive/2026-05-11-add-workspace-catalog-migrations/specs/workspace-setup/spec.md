## ADDED Requirements

### Requirement: Migrate Workspace Catalog persistence before use

The Native Layer SHALL apply Workspace Catalog SQLite schema and persisted Workspace JSON migrations before using the Workspace Catalog for reads, duplicate checks, inserts, or post-insert re-reads.

#### Scenario: Unversioned Workspace Catalog is migrated

- **WHEN** the Native Layer opens an existing Workspace Catalog that has no recorded persistence version
- **THEN** the Native Layer SHALL treat the catalog as version `0`
- **AND** the Native Layer SHALL apply every required migration up to the current persistence version before returning or writing Workspace records
- **AND** the Native Layer SHALL record the current persistence version only after all required migrations complete successfully

#### Scenario: Legacy Workspace JSON is compatible with current bundled catalogs

- **WHEN** a persisted Workspace row contains legacy embedded Workflow Preset, Provisioning Profile, or Endpoint Profile JSON whose selected ids still exist in the current bundled catalogs
- **THEN** the Native Layer SHALL migrate the persisted Workspace JSON into the current domain shape using the current bundled catalog/profile definitions for those selected ids
- **AND** the Native Layer SHALL preserve the Workspace id, name, GPU Cloud Provider id, lifecycle state, selected datacenter, selected GPU, requested persistent storage size, Provider Resource snapshots, and environment preparation timestamp
- **AND** the Native Layer SHALL validate the migrated Workspace record before making it visible as authoritative Workspace Catalog data

#### Scenario: Legacy Workspace JSON cannot be migrated

- **WHEN** a persisted Workspace row cannot be migrated because required selected catalog/profile ids are missing, JSON is malformed, row data is inconsistent, or the migrated Workspace fails domain validation
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative
- **AND** the Native Layer MUST NOT mark the failed migration version as applied

#### Scenario: Current Workspace Catalog is already migrated

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version matches the current application persistence version
- **THEN** the Native Layer SHALL use the existing Workspace records without rewriting them for migration
- **AND** normal Workspace Catalog read, duplicate check, insert, and row consistency validation rules SHALL still apply

#### Scenario: Workspace Catalog was written by a newer app version

- **WHEN** the Native Layer opens a Workspace Catalog whose recorded persistence version is greater than the current application persistence version
- **THEN** the Native Layer SHALL reject the catalog operation with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT read, write, migrate, downgrade, or mutate Workspace records from the newer catalog version

## MODIFIED Requirements

### Requirement: Read Workspace Catalog

The Native Layer SHALL expose a command that returns the local SQLite-backed Workspace Catalog after required Workspace Catalog persistence migrations have completed.

#### Scenario: Workspace Catalog is readable

- **WHEN** the Client requests the Workspace Catalog
- **THEN** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL apply required Workspace Catalog persistence migrations before decoding rows
- **AND** the Native Layer SHALL return all persisted Workspace records known to the local app
- **AND** the Native Layer SHALL verify that each returned persisted Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL treat the returned Workspace Catalog as authoritative durable state

#### Scenario: Workspace Catalog is unavailable

- **WHEN** the Client requests the Workspace Catalog and SQLite initialization, migration, read, decoding, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT return partial Workspace Catalog data as authoritative

### Requirement: Create a Draft Workspace

The Native Layer SHALL expose a command that creates one complete Workspace Catalog entry with lifecycle state `draft` from a client-generated Workspace UUID, name, GPU Cloud Provider id, and full selected Placement Plan. Draft Workspace lifecycle state and empty Provider Resource snapshot state SHALL be authored through the domain Workspace model, and the resulting domain Workspace SHALL be persisted as the authoritative Workspace Catalog record after required Workspace Catalog persistence migrations have completed.

#### Scenario: Valid Workspace creation request

- **WHEN** the Client submits a valid Workspace UUID, non-empty Workspace name, `runpod`, and a valid Placement Plan
- **THEN** the Native Layer SHALL validate the local provider key prerequisite, bundled catalog compatibility, profile compatibility, and placement structure before persistence
- **AND** the Native Layer SHALL initialize the SQLite-backed Workspace Catalog
- **AND** the Native Layer SHALL apply required Workspace Catalog persistence migrations before checking duplicates or writing the new Workspace record
- **AND** the Native Layer SHALL construct the Draft Workspace through the domain Workspace model
- **AND** the domain-authored Workspace SHALL have lifecycle state `draft`
- **AND** the domain-authored Workspace SHALL have empty Persistent Storage Volume, active Provisioning Pod, Serverless Endpoint, and last Provisioning Pod snapshots
- **AND** the Native Layer SHALL persist the domain-authored Workspace as the authoritative Workspace Catalog record
- **AND** the Native Layer SHALL persist one Workspace Catalog entry in SQLite with lifecycle state `draft`
- **AND** the Native Layer SHALL re-read the persisted Workspace record from SQLite
- **AND** the Native Layer SHALL verify that the re-read Workspace record is internally consistent with its indexed SQLite row data
- **AND** the Native Layer SHALL return the re-read Workspace record as authoritative
- **AND** Workspace creation MUST NOT require a live Provider identity check

#### Scenario: Duplicate Workspace UUID

- **WHEN** the Client submits a Workspace UUID that already exists in the Workspace Catalog
- **THEN** the Native Layer SHALL apply required Workspace Catalog persistence migrations before evaluating the duplicate Workspace UUID
- **AND** the Native Layer SHALL reject the request with `workspace_already_exists`
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

- **WHEN** the Client submits a valid Workspace creation request but Workspace Catalog migration, SQLite write, commit, re-read, or row consistency validation fails
- **THEN** the Native Layer SHALL reject the request with `workspace_catalog_unavailable`
- **AND** the Native Layer MUST NOT report Workspace creation success
