## ADDED Requirements

### Requirement: Workspace Resources defines resource-operation error categories

Workspace Resources SHALL define `WorkspaceResourceError` as the resource-operation boundary for catalog/persistence failures, secret/keyring failures, provider API failures, provider resource lifecycle failures, provider operation uncertainty, and Provisioner Worker token lifecycle failures.

#### Scenario: Catalog and persistence failures are preserved

- **WHEN** Workspace Resources fails while loading, updating, or resetting Workspace Catalog state
- **THEN** it SHALL return a catalog/persistence `WorkspaceResourceError` category preserving storage unavailable, migration failed, query failed, corrupt data, schema mismatch, or generic unavailable access
- **AND** it MUST NOT collapse known specific categories into generic Workspace Catalog unavailable behavior

#### Scenario: Secret and keyring failures are categorized

- **WHEN** Workspace Resources fails while reading Provider API Keys or writing, reading, parsing, or deleting Provisioner Worker bearer tokens
- **THEN** it SHALL return a secret/keyring or token lifecycle `WorkspaceResourceError` category
- **AND** it MUST NOT expose Provider API Key values, Provisioner Worker bearer token values, keyring details, or secret storage internals

#### Scenario: Provider API failures are categorized

- **WHEN** a provider capability reports authorization, unavailability, rate limiting, request rejection, response invalidity, conflict, not-found, or indeterminate operation results
- **THEN** Workspace Resources SHALL map the failure into an app-owned `WorkspaceResourceError` category
- **AND** provider-specific response shapes, raw provider details, request bodies, response bodies, and provider error strings MUST remain inside provider implementation boundaries

### Requirement: Workspace Resources preserves recovery-required resource state

Workspace Resources SHALL report provider resource lifecycle failures and uncertainty in a way that allows Workspace Provisioning to persist recovery-required Workspace state.

#### Scenario: Provider operation is indeterminate

- **WHEN** a provider resource operation times out or returns an indeterminate result after resource state may have changed
- **THEN** Workspace Resources SHALL return a provider operation uncertainty category
- **AND** it SHALL preserve any sanitized metadata needed for Workspace Provisioning to persist cleanup recovery state

#### Scenario: Provider resource is missing

- **WHEN** a tracked provider resource is missing during resource observation or cleanup
- **THEN** Workspace Resources SHALL return a provider resource missing category
- **AND** it SHALL preserve known local snapshots needed for later recovery

#### Scenario: Orphaned resources are discovered

- **WHEN** provider discovery finds Workspace-owned or same-name resources that cannot be safely adopted
- **THEN** Workspace Resources SHALL return an orphaned resource category
- **AND** it SHALL include only stable UI-safe metadata suitable for persisted failure details

#### Scenario: Cleanup fails

- **WHEN** provider cleanup or required local token cleanup cannot confirm that known resources and credentials were removed
- **THEN** Workspace Resources SHALL return a cleanup failure category
- **AND** Workspace Provisioning SHALL be able to preserve cleanup metadata for later recovery

### Requirement: Workspace Resources handles provisioning pod token lifecycle safely

Workspace Resources SHALL manage Provisioner Worker bearer token lifecycle around provisioning pod creation according to provider certainty.

#### Scenario: Determinate pod creation failure cleans token

- **WHEN** Workspace Resources stores a per-workspace Provisioner Worker bearer token before creating a provisioning pod
- **AND** provisioning pod creation fails with a determinate result proving that no pod exists
- **THEN** Workspace Resources SHALL attempt best-effort deletion of that Workspace's token
- **AND** it SHALL preserve the original pod creation error category for Workspace Provisioning

#### Scenario: Possible pod state preserves token

- **WHEN** provisioning pod creation is indeterminate or a provider pod may exist
- **THEN** Workspace Resources SHALL preserve the provider uncertainty or possible-resource category
- **AND** it MUST NOT delete the token solely because a local active pod snapshot was not persisted

#### Scenario: Token cleanup failure is secret-safe

- **WHEN** token cleanup fails during provisioning pod creation or cancellation cleanup
- **THEN** Workspace Resources SHALL return a token lifecycle or cleanup category suitable for command mapping or persisted recovery semantics
- **AND** no command response, persisted failure, log, or error metadata may include the token value
