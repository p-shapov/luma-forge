## ADDED Requirements

### Requirement: Report normalized Workspace Provisioning progress
Workspace Provisioning SHALL report `WorkspaceProvisioningProgress.percent` as total provisioning progress toward a Ready Workspace whenever a percentage is present.

#### Scenario: Idle and terminal progress use fixed percentages
- **WHEN** Workspace Provisioning Progress is derived for a Draft Workspace
- **THEN** the progress status SHALL be `idle`
- **AND** the progress phase SHALL be `not_started`
- **AND** the progress percent SHALL be `0`
- **WHEN** Workspace Provisioning Progress is derived for a Ready Workspace
- **THEN** the progress status SHALL be `completed`
- **AND** the progress phase SHALL be `completed`
- **AND** the progress percent SHALL be `100`

#### Scenario: Provider orchestration phases use total-progress anchors
- **WHEN** Workspace Provisioning Progress is derived for active provider orchestration
- **THEN** phase `creating_volume` SHALL report percent `0`
- **AND** phase `starting_provisioning_pod` SHALL report percent `10`
- **AND** phase `creating_endpoint` SHALL report percent `90`
- **AND** phase `validating_readiness` SHALL report percent `98`

#### Scenario: Worker preparation progress is scaled into total progress
- **WHEN** the Provisioner Worker reports running preparation progress for the active Workspace job
- **AND** the worker progress percent is `0`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `40`
- **WHEN** the worker progress percent is `50`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `65`
- **WHEN** the worker progress percent is `100`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `90`

#### Scenario: Missing worker percent uses preparation lower bound
- **WHEN** the Provisioner Worker reports a running preparation status without a progress percentage
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `40`

#### Scenario: Cleanup and failure are outside the ready-progress scale
- **WHEN** Workspace Provisioning Progress is derived for cancellation cleanup
- **THEN** the progress phase SHALL be `cleaning_up`
- **AND** the progress percent SHALL be absent
- **WHEN** Workspace Provisioning Progress is derived for a Failed Workspace
- **THEN** the progress status SHALL be `failed`
- **AND** the progress phase SHALL be `failed`
- **AND** the progress percent SHALL be absent

## MODIFIED Requirements

### Requirement: Drive Provisioner Worker Preparation
Workspace Provisioning SHALL start and observe the Provisioner Worker job using the selected Workflow Preset and a per-workspace bearer token, while treating worker startup lag behind a running Provisioning Pod as non-terminal `starting_provisioning_pod` progress.

#### Scenario: Provisioner Worker is not ready after pod starts
- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot whose provider status is `running`
- **AND** the Provisioner Worker status endpoint is temporarily unreachable, times out, or returns a retryable unavailable or non-worker proxy response while Native can safely continue with the same active pod
- **THEN** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress with status `running`
- **AND** the progress phase SHALL be `starting_provisioning_pod`
- **AND** the progress percent SHALL be `10`
- **AND** the Native Layer MUST NOT mark the Workspace `failed`
- **AND** the Native Layer MUST NOT surface a user-facing `provisioner_worker_unavailable` command error for normal worker readiness lag
- **AND** the Native Layer MUST NOT create another Provisioning Pod

#### Scenario: Provisioner Worker job starts
- **WHEN** the active Provisioning Pod is running and the Provisioner Worker is reachable and idle
- **THEN** the Native Layer SHALL call `POST /start` with the active Workspace identifier as the worker job correlation identifier and the selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request
- **AND** the Native Layer SHALL include the Workspace's resolved runtime image snapshot in the worker start request
- **AND** the Native Layer SHALL treat a worker start response that has not yet reported active preparation work as `starting_provisioning_pod` progress
- **AND** the progress percent SHALL be `10`

#### Scenario: Provisioner Worker idle status is valid
- **WHEN** the Provisioner Worker reports `status` `idle` with no active phase
- **THEN** the Native Layer SHALL treat the response as a valid idle worker status
- **AND** the Native Layer SHALL attempt to start the worker job when the Workspace still requires environment preparation
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because the idle worker response has a null phase

#### Scenario: Provisioner Worker progress is reported
- **WHEN** the Provisioner Worker reports `running` or `cancelling` status for the active Workspace job
- **THEN** the Native Layer SHALL derive Workspace Provisioning Progress from the worker status, phase, and progress percentage
- **AND** the Native Layer SHALL map worker-specific phase names into Workspace Provisioning phases without exposing worker implementation details as durable domain state
- **AND** the Native Layer SHALL scale worker-local preparation percentages into total Workspace Provisioning percentages
- **AND** the Native Layer MUST NOT persist worker progress as authoritative lifecycle state

#### Scenario: Provisioner Worker succeeds
- **WHEN** the Provisioner Worker reports terminal success for the active Workspace job
- **THEN** the Native Layer SHALL persist the environment prepared timestamp
- **AND** later readiness validation SHALL depend on the prepared workspace metadata when that metadata is available through the mounted workspace
- **AND** a terminal success response with no active phase SHALL be treated as valid

#### Scenario: Provisioner Worker fails
- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup
- **AND** returned error metadata SHALL be UI-safe and MUST NOT contain bearer tokens, Provider API Keys, raw command output, stack traces, or environment dumps
- **AND** the Native Layer SHALL preserve stable UI-safe worker error metadata when the worker provides it

#### Scenario: Provisioner Worker API contract error is classified distinctly
- **WHEN** the Provisioner Worker returns an authenticated JSON validation error, malformed worker JSON success payload, unsupported status, unsafe progress percentage, or otherwise unrecoverable API contract response
- **THEN** the Native Layer SHALL classify the failure as a worker response or request contract problem
- **AND** the Native Layer MUST NOT classify that worker JSON response as worker unavailability
- **AND** temporary non-JSON proxy or readiness responses before the worker API is ready SHALL be treated as worker readiness lag rather than worker API contract failures
- **AND** any persisted or returned error metadata SHALL remain UI-safe and secret-safe
