## MODIFIED Requirements

### Requirement: Drive Provisioner Worker Preparation

Workspace Provisioning SHALL start and observe the Provisioner Worker job using the selected Workflow Preset and a per-workspace bearer token, while treating worker startup lag behind a running Provisioning Pod as non-terminal progress.

#### Scenario: Provisioner Worker is not ready after pod starts

- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot whose provider status is `running`
- **AND** the Provisioner Worker status endpoint is temporarily unreachable, times out, or returns a retryable unavailable or non-worker proxy response while Native can safely continue with the same active pod
- **THEN** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress with status `running`
- **AND** the progress phase SHALL indicate that provisioning is still waiting for the worker or preparing the environment
- **AND** the Native Layer MUST NOT mark the Workspace `failed`
- **AND** the Native Layer MUST NOT surface a user-facing `provisioner_worker_unavailable` command error for normal worker readiness lag
- **AND** the Native Layer MUST NOT create another Provisioning Pod

#### Scenario: Provisioner Worker job starts

- **WHEN** the active Provisioning Pod is running and the Provisioner Worker is reachable and idle
- **THEN** the Native Layer SHALL call `POST /start` with the active Workspace identifier as the worker job correlation identifier and the selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request
- **AND** the Native Layer SHALL treat the worker's accepted start response as running preparation progress

#### Scenario: Provisioner Worker idle status is valid

- **WHEN** the Provisioner Worker reports `status` `idle` with no active phase
- **THEN** the Native Layer SHALL treat the response as a valid idle worker status
- **AND** the Native Layer SHALL attempt to start the worker job when the Workspace still requires environment preparation
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because the idle worker response has a null phase

#### Scenario: Provisioner Worker progress is reported

- **WHEN** the Provisioner Worker reports `running` or `cancelling` status for the active Workspace job
- **THEN** the Native Layer SHALL derive Workspace Provisioning Progress from the worker status, phase, progress percentage, and UI-safe diagnostic metadata
- **AND** the Native Layer SHALL map worker-specific phase names into Workspace Provisioning phases without exposing worker implementation details as durable domain state
- **AND** the Native Layer MUST NOT persist worker progress as authoritative lifecycle state

#### Scenario: Provisioner Worker succeeds

- **WHEN** the Provisioner Worker reports terminal success for the active Workspace job
- **THEN** the Native Layer SHALL persist the environment prepared timestamp
- **AND** later readiness validation MAY depend on the prepared environment metadata
- **AND** a terminal success response with no active phase SHALL be treated as valid

#### Scenario: Provisioner Worker fails

- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup
- **AND** returned diagnostics SHALL be UI-safe and MUST NOT contain bearer tokens, Provider API Keys, raw command output, stack traces, or environment dumps
- **AND** the Native Layer SHALL preserve stable UI-safe worker error metadata when the worker provides it

#### Scenario: Provisioner Worker API contract error is classified distinctly

- **WHEN** the Provisioner Worker returns an authenticated JSON validation error, malformed worker JSON success payload, unsupported status, unsafe progress percentage, or otherwise unrecoverable API contract response
- **THEN** the Native Layer SHALL classify the failure as a worker response or request contract problem
- **AND** the Native Layer MUST NOT classify that worker JSON response as worker unavailability
- **AND** temporary non-JSON proxy or readiness responses before the worker API is ready SHALL be treated as worker readiness lag rather than worker API contract failures
- **AND** any persisted or returned diagnostics SHALL remain UI-safe and secret-safe

### Requirement: Record structured provisioning failure details

Workspace Provisioning SHALL persist a structured, UI-safe provisioning failure detail whenever it persists a Workspace lifecycle state of `failed`.

#### Scenario: Terminal provider resource failure is recorded

- **WHEN** a provisioning sync observes a required provider resource in a terminal failed, unexpectedly terminated, unknown, missing, or otherwise unsafe-to-continue state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail with a stable failure code, failed phase, provider-resource source, retryability, and recovery action
- **AND** the Native Layer SHALL retain known Provider Resource snapshots for future cleanup

#### Scenario: Terminal worker failure is recorded

- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error during provisioning
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail with a stable failure code, failed phase, provisioner-worker source, retryability, recovery action, and only sanitized diagnostics
- **AND** the Native Layer SHALL include stable UI-safe worker error code or reason metadata when provided by the worker contract
- **AND** the Native Layer SHALL preserve sanitized diagnostics for both terminal worker job failures and unrecoverable worker API contract failures when the worker provides them
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup

#### Scenario: Unsafe continuation is recorded

- **WHEN** a provider mutation outcome, readiness validation result, local token inconsistency, or cleanup result leaves Native unable to safely continue provisioning without risking duplicate resources, leaked resources, or a false `ready` state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail describing the failed phase, failure source, retryability, and recovery action
- **AND** the Native Layer SHALL retain all known cleanup metadata

#### Scenario: Failed progress includes failure detail

- **WHEN** the Client initiates, syncs, cancels, or reads a Workspace whose lifecycle state is `failed` and whose metadata contains structured provisioning failure detail
- **THEN** the Native Layer SHALL return Workspace Provisioning Progress with status `failed`
- **AND** the returned progress or Workspace payload SHALL expose the structured failure detail through generated binding-safe types
- **AND** React SHALL NOT need to parse a free-form message string to classify the failure

#### Scenario: Legacy failed workspace has no failure detail

- **WHEN** the Client reads or syncs a Workspace whose lifecycle state is `failed` but whose persisted metadata predates structured provisioning failure detail
- **THEN** the Native Layer SHALL return failed progress with a generic UI-safe failure classification
- **AND** the Native Layer MUST NOT infer provider-specific detail that is not present in durable metadata

#### Scenario: Failure details are secret-safe

- **WHEN** the Native Layer records or returns structured provisioning failure detail
- **THEN** the failure detail MUST NOT include Provider API Keys, Provisioner Worker bearer tokens, raw provider responses, provider-specific secret-bearing URLs, raw command output, stack traces, environment dumps, unsanitized worker diagnostics, worker request bodies, or raw worker responses
