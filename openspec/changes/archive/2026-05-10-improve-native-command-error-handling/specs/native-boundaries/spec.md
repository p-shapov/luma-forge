## ADDED Requirements

### Requirement: Command errors expose actionable UI-safe categories

The Native command boundary SHALL expose command errors with stable UI-safe categories that are specific enough for React to render recovery guidance without parsing human-readable messages.

#### Scenario: Command error is returned to React

- **WHEN** a native command fails with a known use-case, provider, storage, catalog, validation, or persistence failure
- **THEN** the generated command error SHALL include a stable `code`
- **AND** the generated command error SHALL include a UI-safe `message`
- **AND** the generated command error SHALL include a `retryable` flag
- **AND** the generated command error MAY include optional UI-safe metadata such as affected field, reason category, or recovery action
- **AND** the generated command error MUST NOT include Provider API Keys, keyring values, raw provider transport bodies, raw GraphQL responses, raw SQLite errors, raw SQL statements, raw `workspace_json`, stack traces, or implementation-only source errors

#### Scenario: Frontend handles command error

- **WHEN** React receives a generated native command error
- **THEN** React SHALL be able to choose user-facing copy and recovery affordances from the error `code` and optional safe metadata
- **AND** React MUST NOT parse `message` to determine recovery behavior

### Requirement: Command error contract is generated and reference-aligned

The Native command error contract SHALL remain generated from the Tauri command boundary and reflected in reference contract documentation.

#### Scenario: Command error codes change

- **WHEN** a native command error code or error metadata field is added, removed, or renamed
- **THEN** generated TypeScript command bindings SHALL be regenerated
- **AND** the reference native contract SHALL be updated to match the generated contract
- **AND** command mapping tests SHALL prove every native use-case error maps to a UI-safe command error

### Requirement: Command boundary preserves precise internal categories until UI-safe mapping

Native implementation boundaries SHALL preserve non-secret failure categories internally until they can be mapped into a UI-safe command error.

#### Scenario: Low-level dependency fails

- **WHEN** provider transport, keyring access, bundled catalog parsing, Workspace Catalog persistence, or request validation fails
- **THEN** the owning native boundary SHALL represent the failure with a typed non-secret category before command mapping
- **AND** immediate mapping to a broad command error SHALL occur only when the detailed category is intentionally not actionable

#### Scenario: Internal error crosses command boundary

- **WHEN** an internal error reaches a Tauri command handler
- **THEN** the command handler or command-adjacent mapper SHALL convert it into the generated command error contract
- **AND** the application service MUST NOT return the command-owned DTO directly
