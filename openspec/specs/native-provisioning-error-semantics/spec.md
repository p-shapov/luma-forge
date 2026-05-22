# native-provisioning-error-semantics Specification

## Purpose
TBD - created by archiving change native-provisioning-error-semantics. Update Purpose after archive.
## Requirements
### Requirement: Native provisioning distinguishes command errors from persisted failures

The Native Layer SHALL classify provisioning and resource-operation failures as either immediate command errors or persisted `WorkspaceProvisioningFailure` records according to whether durable Workspace recovery state is required.

#### Scenario: Local persistence failure is returned as command error

- **WHEN** Workspace Provisioning or Workspace Resources encounters SQLite storage, query, migration, catalog corruption, or catalog schema mismatch failure before it can safely mutate provider resources
- **THEN** the Native Layer SHALL return a granular UI-safe command error
- **AND** the Native Layer MUST NOT persist a new `WorkspaceProvisioningFailure` for that local persistence failure
- **AND** the Native Layer MUST NOT create, delete, or modify provider resources as part of that failed operation

#### Scenario: Provider resource uncertainty is persisted

- **WHEN** a provider mutation, observation, or cleanup leaves provider resource state unsafe, missing, orphaned, indeterminate, or requiring cleanup recovery
- **THEN** the Native Layer SHALL persist a structured `WorkspaceProvisioningFailure` on the Workspace when catalog persistence is available
- **AND** the persisted failure SHALL include stable UI-safe code, phase, source, and recovery action
- **AND** the Native Layer MUST NOT hide the recovery-required state behind a generic command error

#### Scenario: Provider availability blocking provisioning is persisted

- **WHEN** provider API unavailability, rate limiting, operation conflict, or request rejection prevents an active provisioning sync from completing
- **THEN** the Native Layer SHALL persist a structured `WorkspaceProvisioningFailure` on the Workspace when catalog persistence is available
- **AND** the Native Layer SHALL transition the Workspace lifecycle state to `failed` when catalog persistence is available
- **AND** when catalog persistence is available, the persisted failure recovery action SHALL be the durable UI recovery signal instead of a retryable flag

#### Scenario: Worker readiness lag remains progress

- **WHEN** a Provisioner Worker is temporarily unreachable while the active provisioning pod can still be safely retried or observed
- **THEN** the Native Layer SHALL keep Workspace Provisioning in running or readiness progress
- **AND** the Native Layer MUST NOT persist a failure or return a user-facing worker unavailable command error for normal readiness lag

#### Scenario: Terminal worker preparation failure is persisted

- **WHEN** a Provisioner Worker reports a terminal preparation failure during active provisioning sync
- **THEN** the Native Layer SHALL persist a structured `WorkspaceProvisioningFailure` on the Workspace when catalog persistence is available
- **AND** the persisted failure SHALL include the most granular stable worker failure code available
- **AND** the persisted failure source SHALL be `provisioner_worker`
- **AND** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress when the failure is persisted
- **AND** the Native Layer MUST NOT return `NativeCommandErrorCode::ProvisionerWorkerFailed` as the normal result for the terminal worker subtype

### Requirement: Resource-operation errors preserve boundary categories

Workspace Resources SHALL classify resource-operation failures into app-owned categories for catalog/persistence failures, secret/keyring failures, provider API failures, provider resource lifecycle failures, provider operation uncertainty, and Provisioner Worker token lifecycle failures.

#### Scenario: Resource catalog failure is categorized

- **WHEN** Workspace Resources cannot load, persist, or reset Workspace Catalog state
- **THEN** it SHALL return a `WorkspaceResourceError` category that preserves whether the failure is storage unavailable, migration failed, query failed, corrupt catalog data, schema mismatch, or generic catalog unavailable

#### Scenario: Resource secret failure is categorized

- **WHEN** Workspace Resources cannot read, write, parse, or delete a Provider API Key or Provisioner Worker bearer token through secure storage
- **THEN** it SHALL return a `WorkspaceResourceError` category for the secret/keyring or token lifecycle failure
- **AND** returned, logged, or persisted data MUST NOT include the secret value

#### Scenario: Resource provider failure is categorized

- **WHEN** Workspace Resources receives provider-local API, request, response, lifecycle, not-found, rate-limit, unavailable, conflict, or indeterminate failures
- **THEN** it SHALL map them into app-owned `WorkspaceResourceError` categories
- **AND** it MUST NOT expose provider-specific response envelopes, raw request bodies, raw response bodies, bearer headers, or provider-specific error strings outside the provider/resource boundary

### Requirement: Provisioning orchestration maps resource errors by recovery semantics

Workspace Provisioning SHALL map `WorkspaceResourceError` into `WorkspaceProvisioningError`, non-mutating progress, or persisted `WorkspaceProvisioningFailure` according to the provisioning phase and recovery semantics.

#### Scenario: Resource error escapes as command error

- **WHEN** a resource-operation failure is local, pre-provisioning, or otherwise does not require durable Workspace recovery state
- **THEN** Workspace Provisioning SHALL map it to a `WorkspaceProvisioningError` that the command boundary can return as a UI-safe command error
- **AND** Workspace Provisioning MUST NOT persist a `WorkspaceProvisioningFailure` solely for that escaped command error

#### Scenario: Resource error becomes persisted failure

- **WHEN** a resource-operation failure means provider state may be unsafe, remote resources are missing or orphaned, cleanup is incomplete, or worker/token state requires inspection or recovery
- **THEN** Workspace Provisioning SHALL persist a structured `WorkspaceProvisioningFailure` when catalog persistence is available
- **AND** command responses and Workspace progress SHALL expose the persisted failure through generated binding-safe types

#### Scenario: Terminal worker subtype becomes persisted failure

- **WHEN** Workspace Provisioning receives terminal worker preparation subtype `ProvisionerWorkerAssetDownloadFailed`, `ProvisionerWorkerAssetAuthRequired`, `ProvisionerWorkerPathValidationFailed`, `ProvisionerWorkerStepTimeout`, or `ProvisionerWorkerUnexpectedError` during active provisioning sync
- **THEN** Workspace Provisioning SHALL persist the corresponding granular `WorkspaceProvisioningFailureCode` when catalog persistence is available
- **AND** Workspace Provisioning SHALL return authoritative failed Workspace state and failed progress after persistence succeeds
- **AND** Workspace Provisioning MUST NOT rely on the command boundary's generic `provisioner_worker_failed` fallback for normal sync handling

#### Scenario: Defensive command mapping remains available

- **WHEN** a terminal worker preparation subtype reaches the command error boundary despite normal provisioning sync handling
- **THEN** the command boundary SHALL return a UI-safe `provisioner_worker_failed` command error
- **AND** the command error MUST NOT expose bearer tokens, Provider API Keys, raw worker output, stack traces, environment dumps, or provider transport details

#### Scenario: Mapping is regression tested

- **WHEN** regression tests exercise `WorkspaceResourceError -> WorkspaceProvisioningError`, `WorkspaceProvisioningError -> WorkspaceProvisioningFailure`, and `WorkspaceProvisioningError -> NativeCommandError` conversions
- **THEN** catalog, secret/keyring, provider API, provider uncertainty, resource lifecycle, worker, worker subtype, and token lifecycle categories SHALL map to the expected command-error, persisted-failure, non-mutating progress, or defensive fallback behavior

### Requirement: Worker token lifecycle respects provider certainty

The Native Layer SHALL manage Provisioner Worker bearer token cleanup according to provisioning pod creation certainty and environment-preparation state.

#### Scenario: Determinate pod creation failure cleans token

- **WHEN** the per-workspace Provisioner Worker bearer token is stored
- **AND** provider provisioning pod creation fails with a determinate result proving no provider pod was created
- **THEN** Workspace Resources SHALL attempt best-effort deletion of that Workspace's stored token
- **AND** the original pod creation failure category SHALL remain visible to Workspace Provisioning

#### Scenario: Indeterminate pod creation preserves token for recovery

- **WHEN** provider provisioning pod creation is indeterminate or a provider pod may exist
- **THEN** the Native Layer SHALL preserve recovery metadata for the possible provider pod
- **AND** it MUST NOT delete the worker token solely because no local active pod snapshot has been persisted yet

#### Scenario: Token missing during environment preparation is persisted

- **WHEN** Workspace Provisioning needs a stored Provisioner Worker bearer token to contact an active provisioning pod during environment preparation
- **AND** the token is missing or invalid
- **THEN** Workspace Provisioning SHALL persist a structured failure indicating native state inconsistency
- **AND** the persisted failure and command responses MUST NOT expose the token value
