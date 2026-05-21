## ADDED Requirements

### Requirement: Workspace Catalog failures remain categorized

Workspace Setup SHALL classify Workspace Catalog failures into stable app-owned categories for storage unavailable, migration failed, query failed, corrupt catalog data, schema mismatch, and generic unavailable catalog access.

#### Scenario: Storage is unavailable

- **WHEN** Workspace Setup cannot initialize or access the Workspace Catalog because local storage is unavailable
- **THEN** Workspace Setup SHALL return the Workspace Catalog storage unavailable category
- **AND** Workspace Setup MUST NOT expose raw filesystem, SQLite, or platform details through command-facing data

#### Scenario: Migration or compatibility check fails

- **WHEN** Workspace Setup rejects a Workspace Catalog because schema bootstrap, migration, version compatibility, or downgrade checks fail
- **THEN** Workspace Setup SHALL return the Workspace Catalog migration failed category
- **AND** Workspace Setup MUST NOT read, write, migrate, downgrade, or return Workspace records from the rejected catalog

#### Scenario: Catalog query fails

- **WHEN** Workspace Setup cannot complete a Workspace Catalog read, duplicate check, insert, update, or re-read query
- **THEN** Workspace Setup SHALL return the Workspace Catalog query failed category
- **AND** Workspace Setup MUST NOT return partial Workspace Catalog data as authoritative

#### Scenario: Catalog data is corrupt

- **WHEN** Workspace Setup reads normalized Workspace Catalog data that cannot be decoded or reconstructed into a valid Workspace
- **THEN** Workspace Setup SHALL return the Workspace Catalog corrupt category
- **AND** Workspace Setup MUST NOT return partial Workspace Catalog data as authoritative

#### Scenario: Catalog schema mismatches expected shape

- **WHEN** Workspace Setup finds Workspace Catalog tables, columns, indexes, or related rows that do not match the expected current schema or violate persisted schema invariants
- **THEN** Workspace Setup SHALL return the Workspace Catalog schema mismatch category
- **AND** Workspace Setup MUST NOT collapse the failure into a generic unavailable category

#### Scenario: Generic unavailable fallback is used

- **WHEN** Workspace Setup cannot classify a Workspace Catalog access failure into a more specific app-owned category
- **THEN** Workspace Setup MAY return the generic Workspace Catalog unavailable category
- **AND** the command boundary SHALL still map it to UI-safe recovery metadata
