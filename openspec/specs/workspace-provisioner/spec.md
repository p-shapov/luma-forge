# workspace-provisioner Specification

## Purpose
TBD - created by archiving change extract-workspace-provisioner. Update Purpose after archive.
## Requirements
### Requirement: Orchestrate workspace environment preparation
The Native Layer SHALL provide a workspace provisioner boundary that drives environment preparation for a provisioning Workspace through the existing Provisioner Worker without owning provider-resource lifecycle.

#### Scenario: Environment preparation waits for running provisioning pod
- **WHEN** Workspace Provisioning asks the workspace provisioner to sync environment preparation for a Workspace whose environment is not yet prepared
- **AND** the Workspace has no active Provisioning Pod snapshot or the active Provisioning Pod is not running
- **THEN** the workspace provisioner SHALL return without starting a worker job
- **AND** it MUST NOT create, observe, or delete provider resources

#### Scenario: Idle worker is started
- **WHEN** Workspace Provisioning asks the workspace provisioner to sync environment preparation for a Workspace with a running active Provisioning Pod
- **AND** the stored per-workspace Provisioner Worker bearer token is readable
- **AND** the Provisioner Worker reports `idle`
- **THEN** the workspace provisioner SHALL start the worker job using the Workspace identifier as the job correlation identifier
- **AND** the start request SHALL include the selected Workflow Preset and resolved runtime image snapshot
- **AND** the start request MUST NOT include Provider API Keys or worker bearer token values in any persisted Workspace metadata

#### Scenario: Worker progress is returned without durable progress mutation
- **WHEN** the Provisioner Worker reports non-terminal preparation progress
- **THEN** the workspace provisioner SHALL return Workspace Provisioning Progress derived from the worker status
- **AND** it SHALL return the authoritative Workspace metadata
- **AND** it MUST NOT persist worker progress as durable Workspace lifecycle state

#### Scenario: Worker success marks environment prepared
- **WHEN** the Provisioner Worker reports terminal success for the active Workspace preparation job
- **THEN** the workspace provisioner SHALL persist the Workspace environment prepared timestamp
- **AND** it SHALL return the authoritative persisted Workspace metadata
- **AND** it MUST NOT terminate the temporary provisioning pod

#### Scenario: Worker readiness lag remains non-terminal
- **WHEN** the active Provisioning Pod is running
- **AND** the Provisioner Worker status endpoint is temporarily unreachable, times out, or returns a retryable unavailable or non-worker proxy response
- **THEN** the workspace provisioner SHALL return running Workspace Provisioning Progress for environment preparation
- **AND** it MUST NOT mark the Workspace failed
- **AND** it MUST NOT create another Provisioning Pod

#### Scenario: Worker token failure is persisted as workspace failure
- **WHEN** the stored per-workspace Provisioner Worker bearer token is missing or invalid while the Workspace requires environment preparation
- **THEN** the workspace provisioner SHALL mark the Workspace failed with structured Provisioner Worker failure detail
- **AND** it SHALL persist the failed Workspace before reporting the result
- **AND** it MUST NOT expose the missing, invalid, or expected token value in Workspace metadata, command responses, logs, or error metadata

#### Scenario: Worker terminal failure is persisted as workspace failure
- **WHEN** the Provisioner Worker reports a terminal failure or an invalid non-retryable worker response while the Workspace requires environment preparation
- **THEN** the workspace provisioner SHALL mark the Workspace failed with structured Provisioner Worker failure detail
- **AND** it SHALL preserve UI-safe worker error metadata when available
- **AND** it MUST NOT include secrets, request bodies, bearer headers, or credential-bearing URLs in the persisted failure detail
