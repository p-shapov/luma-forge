## MODIFIED Requirements

### Requirement: Cancel Workspace Provisioning

Workspace Provisioning SHALL support user cancellation while a Workspace is in `provisioning` by using shared known-resource cleanup behavior and returning the Workspace to `draft` only after cancellation cleanup succeeds.

#### Scenario: Cancellation succeeds

- **WHEN** the Client cancels provisioning for a Workspace in `provisioning`
- **THEN** the Native Layer SHALL invoke shared cleanup behavior for the Workspace-owned Provider Resources known from authoritative Workspace metadata
- **AND** shared cleanup SHALL delete the Serverless Endpoint, RunPod endpoint template, Provisioning Pod, and Persistent Storage Volume resources known from Workspace metadata when they exist
- **AND** shared cleanup MUST NOT call the Provisioner Worker `/cancel` endpoint during destructive Workspace Provisioning cancellation
- **AND** the Native Layer SHALL tolerate already-missing provider resources
- **AND** the Native Layer SHALL clear provisioning snapshots and return the Workspace lifecycle state to `draft` only after cleanup is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token when no active provisioning pod remains

#### Scenario: Cancellation skips worker cancel even when worker metadata exists

- **WHEN** the Client cancels provisioning for a Workspace with an active Provisioning Pod snapshot and Provisioner Worker token
- **AND** the active Provisioning Pod snapshot contains a Provisioner Worker status URL
- **AND** Native deletes or confirms missing all known Provider Resources
- **AND** Native deletes the stored Provisioner Worker bearer token when no active provisioning pod remains
- **THEN** the Native Layer SHALL return the Workspace lifecycle state to `draft`
- **AND** the Native Layer MUST NOT call the Provisioner Worker `/cancel` endpoint
- **AND** the Native Layer MUST NOT persist `cancellation_cleanup_failed` for any worker cancellation outcome because worker cancellation is not part of destructive cancellation cleanup

#### Scenario: Cancellation cleanup is incomplete

- **WHEN** cancellation cannot confirm deletion of all known Provider Resources
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain all known Provider Resource and RunPod template metadata for future Workspace Resource Cleanup

#### Scenario: Shared cleanup preserves policy-specific final mutation

- **WHEN** shared known-resource cleanup succeeds for a cancellation request
- **THEN** Workspace Provisioning SHALL apply the cancellation policy by clearing provisioning snapshots and returning the existing Workspace Catalog entry to `draft`
- **AND** shared cleanup behavior MUST NOT delete the Workspace Catalog entry during provisioning cancellation

### Requirement: Reuse Known Workspace Resource Cleanup

The Native Layer SHALL centralize deletion of known Workspace-owned provisioning resources so Workspace Provisioning cancellation and future Workspace Resource Cleanup use the same provider deletion semantics.

#### Scenario: Known resources are cleaned in dependency-safe order

- **WHEN** shared cleanup receives Workspace metadata with known provisioning resources
- **THEN** it SHALL attempt provider cleanup in dependency-safe order: Serverless Endpoint, RunPod endpoint template, active Provisioning Pod, and Persistent Storage Volume
- **AND** it MUST NOT attempt to cancel the active Provisioner Worker job during destructive Workspace Provisioning cancellation
- **AND** it SHALL delete the per-workspace Provisioner Worker bearer token after the active Provisioning Pod is deleted or confirmed missing
- **AND** it SHALL tolerate already-missing Provider Resources
- **AND** it SHALL report cleanup success only after all known Provider Resources are deleted or confirmed missing and required local cleanup succeeds

#### Scenario: Cleanup final state is chosen by caller policy

- **WHEN** shared cleanup returns a result to a caller
- **THEN** the caller SHALL decide the final local Workspace Catalog mutation
- **AND** Workspace Provisioning cancellation SHALL return the Workspace to `draft` on cleanup success
- **AND** future Workspace Resource Cleanup MAY delete the Workspace Catalog entry on cleanup success
- **AND** cleanup failure SHALL preserve known metadata for later recovery
