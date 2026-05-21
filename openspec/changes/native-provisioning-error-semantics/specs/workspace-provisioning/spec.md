## ADDED Requirements

### Requirement: Provisioning preserves Workspace Catalog error categories

Workspace Provisioning SHALL preserve Workspace Catalog error categories received from Workspace Setup, Workspace Catalog repository operations, or Workspace Resources instead of collapsing them into generic catalog unavailability.

#### Scenario: Initiate provisioning sees specific catalog error

- **WHEN** initiating Workspace Provisioning fails because loading or updating the Workspace Catalog returns storage unavailable, migration failed, query failed, corrupt data, or schema mismatch
- **THEN** Workspace Provisioning SHALL return the corresponding provisioning error category as an immediate command failure
- **AND** the Native Layer MUST NOT create, modify, or delete Provider Resources
- **AND** the Native Layer MUST NOT persist a new Workspace failure for the catalog failure

#### Scenario: Sync provisioning sees specific catalog error

- **WHEN** syncing Workspace Provisioning fails because loading or updating the Workspace Catalog returns storage unavailable, migration failed, query failed, corrupt data, or schema mismatch
- **THEN** Workspace Provisioning SHALL return the corresponding provisioning error category as an immediate command failure
- **AND** Workspace Provisioning MUST NOT hide the original catalog category behind generic Workspace Catalog unavailable behavior

### Requirement: Provisioning persists recovery-required resource failures

Workspace Provisioning SHALL persist `WorkspaceProvisioningFailure` records when resource-operation failures require user inspection, cleanup, or durable recovery state.

#### Scenario: Provider operation is indeterminate

- **WHEN** a provider resource create, observe, or cleanup operation is indeterminate and provider resource state may be unsafe
- **THEN** Workspace Provisioning SHALL persist a structured failure with provider-resource source and cleanup-oriented recovery action
- **AND** Workspace Provisioning MUST NOT return only a generic command error for the unsafe state

#### Scenario: Tracked provider resource is missing

- **WHEN** a tracked provider resource is missing during observation or cleanup
- **THEN** Workspace Provisioning SHALL persist a structured failure with provider-resource source and cleanup-oriented recovery action
- **AND** known Workspace metadata and cleanup metadata SHALL be retained

#### Scenario: Orphaned provider resources are discovered

- **WHEN** provider discovery finds Workspace-owned or same-name provider resources that cannot be safely adopted
- **THEN** Workspace Provisioning SHALL persist a structured failure with cleanup-oriented recovery action and stable UI-safe failure metadata
- **AND** Workspace Provisioning MUST NOT adopt the resource or create a duplicate resource

#### Scenario: Cancellation cleanup is incomplete

- **WHEN** cancellation cannot confirm deletion of all known provider resources or required local Provisioner Worker token cleanup
- **THEN** Workspace Provisioning SHALL persist a structured failure with cleanup-oriented recovery action
- **AND** Workspace Provisioning MUST NOT return the Workspace to `draft`

### Requirement: Provisioning handles provider and worker transient failures by phase

Workspace Provisioning SHALL return command errors, non-mutating progress, or persisted failures for provider and worker failures according to phase-specific recovery semantics.

#### Scenario: Provider API is unavailable or rate limited before unsafe mutation

- **WHEN** provider API unavailability or rate limiting occurs before provider state becomes unsafe
- **THEN** Workspace Provisioning SHALL return a retryable command error or non-mutating progress according to the current phase
- **AND** Workspace Provisioning MUST NOT persist a failure solely for the transient provider condition

#### Scenario: Provider request is rejected or response is invalid

- **WHEN** a provider request is rejected or a provider response is invalid
- **THEN** Workspace Provisioning SHALL return a command error or persist a structured failure according to whether the current phase has created unsafe provider/resource state
- **AND** the chosen behavior SHALL preserve stable UI-safe reason and recovery action metadata

#### Scenario: Worker readiness lag is non-terminal

- **WHEN** the Provisioner Worker is temporarily unreachable while Native can safely continue observing the active provisioning pod
- **THEN** Workspace Provisioning SHALL continue reporting running or readiness progress
- **AND** Workspace Provisioning MUST NOT persist failure state for normal worker startup lag

#### Scenario: Worker terminal or contract failure is persisted

- **WHEN** the Provisioner Worker is unauthorized, returns an invalid unrecoverable response, reports terminal failure, or otherwise violates the worker API contract during environment preparation
- **THEN** Workspace Provisioning SHALL persist a structured failure with provisioner-worker source and inspect-oriented or recovery-oriented action
- **AND** persisted failure metadata MUST remain stable, UI-safe, and secret-safe

#### Scenario: Worker token is missing or invalid during preparation

- **WHEN** Workspace Provisioning needs a stored Provisioner Worker bearer token to communicate with an active provisioning pod
- **AND** the stored token is missing or invalid
- **THEN** Workspace Provisioning SHALL persist a structured failure as native state inconsistency
- **AND** command responses and persisted failure metadata MUST NOT include the token value

### Requirement: Provisioning maps resource errors explicitly

Workspace Provisioning SHALL explicitly map `WorkspaceResourceError` categories into immediate command errors, non-mutating progress, or persisted `WorkspaceProvisioningFailure` records.

#### Scenario: Resource error escapes as command error

- **WHEN** Workspace Resources returns a catalog, secret/keyring, transient provider availability, conflict, or other category that does not require durable Workspace recovery state
- **THEN** Workspace Provisioning SHALL map it to the corresponding `WorkspaceProvisioningError`
- **AND** the command boundary SHALL map it into stable UI-safe command metadata

#### Scenario: Resource error becomes persisted failure

- **WHEN** Workspace Resources returns provider operation uncertainty, provider resource missing, orphaned resource, cleanup failure, terminal worker failure, or token lifecycle state inconsistency
- **THEN** Workspace Provisioning SHALL persist the corresponding structured Workspace failure when catalog persistence is available
- **AND** Workspace progress SHALL expose the persisted failure through generated binding-safe types

#### Scenario: Mapping behavior is covered by tests

- **WHEN** regression tests exercise representative `WorkspaceResourceError` categories
- **THEN** each category SHALL assert whether it returns as a command error or persists as Workspace failure
