## MODIFIED Requirements

### Requirement: Start provisioning from selected Workflow Preset

The Provisioner Worker SHALL start one provisioning job only after `POST /start` receives a selected Workflow Preset payload and job correlation identifier.

#### Scenario: Start request is accepted

- **WHEN** `POST /start` receives a valid `job_id` and selected Workflow Preset while the worker is idle
- **THEN** the Provisioner Worker SHALL create one active provisioning job correlated by that `job_id`
- **AND** the Provisioner Worker SHALL begin preparing the configured mounted workspace volume
- **AND** `GET /status` SHALL report `running` with the active job identifier
- **AND** the accepted start response SHALL use the same status payload shape as `GET /status`

#### Scenario: Start request is invalid

- **WHEN** `POST /start` receives a missing job identifier, missing selected Workflow Preset, unsupported source type, or unsafe install path
- **THEN** the Provisioner Worker SHALL reject the request
- **AND** the Provisioner Worker MUST remain idle
- **AND** the Provisioner Worker MUST NOT write to the mounted workspace volume
- **AND** the error response SHALL use the standard worker error payload shape with `code`, `reason_code`, and `message`

#### Scenario: Start request is concurrent

- **WHEN** `POST /start` is called while a provisioning job is active
- **THEN** the Provisioner Worker SHALL reject the request with a conflict error
- **AND** the Provisioner Worker MUST NOT start, queue, or replace a second job
- **AND** the active job SHALL continue unless separately cancelled

### Requirement: Report provisioning status

The Provisioner Worker SHALL report UI-safe provisioning job status through `GET /status`.

#### Scenario: Worker is idle

- **WHEN** no provisioning job has been started
- **THEN** `GET /status` SHALL return status `idle`
- **AND** the response SHALL include no active job identifier
- **AND** the response MAY report no active phase
- **AND** the response MUST NOT include secrets, request bodies, raw command output, stack traces, or environment dumps

#### Scenario: Job is running

- **WHEN** a provisioning job is active
- **THEN** `GET /status` SHALL return the active job identifier, status `running`, current phase, updated timestamp, and optional progress percentage
- **AND** the current phase SHALL use a stable worker phase value for the active preparation step
- **AND** the response MAY include a UI-safe diagnostic message

#### Scenario: Job succeeds

- **WHEN** ComfyUI, Custom Nodes, model assets, and final validation complete successfully
- **THEN** the Provisioner Worker SHALL mark the job `succeeded`
- **AND** `GET /status` SHALL report terminal success
- **AND** the terminal success response MAY report no active phase

#### Scenario: Job fails

- **WHEN** a provisioning step cannot complete safely
- **THEN** the Provisioner Worker SHALL mark the active job `failed`
- **AND** `GET /status` SHALL report terminal failure with UI-safe error metadata
- **AND** the terminal error metadata SHALL use the standard worker error payload shape with `code`, `reason_code`, and `message`
- **AND** the response MAY include a UI-safe diagnostic message
- **AND** the response MUST NOT include provider secrets, tokens, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs

#### Scenario: Job is cancelling or cancelled

- **WHEN** cancellation has been requested for an active job
- **THEN** `GET /status` SHALL report `cancelling` until active work has stopped
- **AND** the Provisioner Worker SHALL report `cancelled` after cancellation completes
- **AND** terminal cancelled status MAY report no active phase
