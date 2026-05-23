# workspace-resources-provider-polymorphism Specification

## Purpose
TBD - created by archiving change refactor-workspace-resources-provider-polymorphism. Update Purpose after archive.
## Requirements
### Requirement: Workspace Resources exposes explicit resource operations
Workspace Resources SHALL expose provider-neutral resource operation methods whose names describe one requested provider resource mutation or observation.

#### Scenario: Resource operation is explicit
- **WHEN** Workspace Provisioning needs provider resource work
- **THEN** it SHALL call an explicit Workspace Resources operation such as create, observe, delete, or cleanup
- **AND** Workspace Resources MUST NOT expose provider-neutral `sync_*` methods that derive the next provisioning step from Workspace state

#### Scenario: Resources do not own provisioning state machine
- **WHEN** Workspace Resources receives a Workspace for an explicit operation
- **THEN** it SHALL use the Workspace only for resource operation inputs, provider identity, and resource snapshot persistence
- **AND** it MUST NOT choose the next provisioning phase, mutate Workspace lifecycle state, write provisioning progress, or persist `last_provisioning_failure`

### Requirement: Serverless endpoint snapshots carry optional provider metadata
Workspace Resources SHALL persist provider-specific serverless endpoint metadata on the serverless endpoint snapshot when that metadata is needed to manage or clean up the endpoint.

#### Scenario: Provider does not need endpoint metadata
- **WHEN** a provider serverless endpoint has no provider-specific metadata needed by Native
- **THEN** the serverless endpoint snapshot provider metadata SHALL be absent

#### Scenario: RunPod endpoint metadata is persisted
- **WHEN** RunPod Workspace Resources creates and persists a serverless endpoint snapshot
- **THEN** the snapshot SHALL include RunPod provider metadata containing the endpoint template identifier needed for cleanup
- **AND** the metadata MUST NOT expose raw RunPod request bodies, response payloads, Provider API Keys, or worker bearer tokens

### Requirement: Workspace Resources uses service-level provider capability dispatch
Workspace Resources SHALL select provider-specific resource behavior through a narrow Workspace Resources provider capability selected by `GpuCloudProviderId`.

#### Scenario: RunPod resource capability is selected
- **WHEN** Workspace Resources needs provider-specific behavior for `runpod`
- **THEN** the Workspace Resources provider registry SHALL return the concrete RunPod Workspace Resources provider capability
- **AND** shared Workspace Resources orchestration SHALL NOT directly call a provider-specific client or match provider request/response DTOs outside the registry/capability layer

#### Scenario: Provider-specific resource behavior remains concrete
- **WHEN** the RunPod Workspace Resources provider capability creates, observes, deletes, or cleans up provider resources
- **THEN** it SHALL use the concrete RunPod client through RunPod-specific provider code
- **AND** RunPod request and response shapes MUST remain in `provider/runpod` or RunPod-specific Workspace Resources modules
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers
- **AND** RunPod-specific endpoint template handling MUST remain inside RunPod Workspace Resources and MUST NOT be exposed as a provider-neutral Workspace Resources operation

### Requirement: Workspace Resources preserves shared lifecycle behavior
Workspace Resources SHALL keep provider-neutral resource snapshot persistence, resource cleanup, UI-safe resource error semantics, and secret isolation in the shared Workspace Resources service.

#### Scenario: Shared resource behavior remains provider-neutral
- **WHEN** Workspace Resources performs an explicit resource operation requested by Workspace Provisioning
- **THEN** workspace catalog persistence, UI-safe resource error semantics, resource snapshot construction, and secret isolation SHALL remain stable
- **AND** provider-specific capability implementations SHALL NOT expose provider secrets, raw provider request bodies, or raw provider response bodies
- **AND** Workspace Resources MUST NOT set `WorkspaceLifecycleState`, write `last_provisioning_failure`, choose `WorkspaceProvisioningPhase`, or derive Workspace Provisioning progress

#### Scenario: Resource cleanup preserves snapshot cleanup semantics
- **WHEN** provider cleanup succeeds for known workspace resources
- **THEN** the shared Workspace Resources service SHALL clear provider resource snapshots after provider cleanup
- **AND** the shared Workspace Resources service SHALL persist the snapshot cleanup before reporting success
- **AND** the shared Workspace Resources service MUST NOT change Workspace lifecycle state or provisioning failure state
- **AND** the production RunPod cleanup behavior SHALL continue deleting known endpoint, endpoint template, provisioning pod, volume, and per-workspace worker token resources

### Requirement: Workspace Resources defines resource-operation error categories
Workspace Resources SHALL define `WorkspaceResourceError` as the resource-operation boundary for catalog/persistence failures, secret/keyring failures, provider API failures, provider resource lifecycle failures, provider operation uncertainty, orphaned provider resources, cleanup failures, and Provisioner Worker token lifecycle failures.

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

#### Scenario: Orphaned provider resources are categorized
- **WHEN** provider discovery finds Workspace-owned or same-name resources that cannot be safely adopted by the requested resource operation
- **THEN** Workspace Resources SHALL return an app-owned orphaned resource `WorkspaceResourceError` category
- **AND** Workspace Resources MUST NOT persist a provisioning failure for that orphaned resource condition

### Requirement: Workspace Resources preserves recovery-required resource state
Workspace Resources SHALL report provider resource lifecycle failures and uncertainty in a way that allows Workspace Provisioning to persist recovery-required Workspace state.

#### Scenario: Provider operation is indeterminate
- **WHEN** a provider resource operation times out or returns an indeterminate result after resource state may have changed
- **THEN** Workspace Resources SHALL return a provider operation uncertainty category
- **AND** it SHALL preserve any sanitized resource snapshots needed for Workspace Provisioning to persist cleanup recovery state
- **AND** it MUST NOT persist a provisioning failure directly

#### Scenario: Provider resource is missing
- **WHEN** a tracked provider resource is missing during resource observation or cleanup
- **THEN** Workspace Resources SHALL return a provider resource missing category
- **AND** it SHALL preserve known local snapshots needed for later recovery
- **AND** it MUST NOT persist a provisioning failure directly

#### Scenario: Orphaned resources are discovered
- **WHEN** provider discovery finds Workspace-owned or same-name resources that cannot be safely adopted
- **THEN** Workspace Resources SHALL return an orphaned resource category
- **AND** it SHALL include only stable UI-safe metadata suitable for provisioning recovery decisions
- **AND** it MUST NOT persist a provisioning failure directly

#### Scenario: Cleanup fails
- **WHEN** provider cleanup or required local token cleanup cannot confirm cleanup of known resources and credentials
- **THEN** Workspace Resources SHALL return a cleanup failure category
- **AND** Workspace Provisioning SHALL be able to preserve cleanup metadata for later recovery
- **AND** Workspace Resources MUST NOT set Workspace lifecycle state or provisioning failure state

### Requirement: Workspace Resources handles provisioning pod token lifecycle safely
Workspace Resources SHALL manage Provisioner Worker bearer token lifecycle around provisioning pod creation according to provider certainty.

#### Scenario: Determinate pod creation failure cleans token
- **WHEN** Workspace Resources stores a per-workspace Provisioner Worker bearer token before creating a provisioning pod
- **AND** provisioning pod creation fails with a determinate result proving that no pod exists
- **THEN** Workspace Resources SHALL attempt best-effort deletion of that Workspace's token
- **AND** it SHALL preserve the original pod creation error category for Workspace Provisioning
- **AND** it MUST NOT persist a provisioning failure directly

#### Scenario: Possible pod state preserves token
- **WHEN** provisioning pod creation is indeterminate or a provider pod may exist
- **THEN** Workspace Resources SHALL preserve the provider uncertainty or possible-resource category
- **AND** it MUST NOT delete the token solely because a local active pod snapshot was not persisted
- **AND** it MUST NOT persist a provisioning failure directly

#### Scenario: Token cleanup failure is secret-safe
- **WHEN** token cleanup fails during provisioning pod creation or cancellation cleanup
- **THEN** Workspace Resources SHALL return a token lifecycle or cleanup category suitable for command mapping or persisted recovery semantics
- **AND** no command response, persisted failure, log, or error metadata may include the token value
