## ADDED Requirements

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
- **THEN** the failure detail MUST NOT include Provider API Keys, Provisioner Worker bearer tokens, raw provider responses, provider-specific secret-bearing URLs, raw command output, stack traces, environment dumps, or unsanitized worker diagnostics

## MODIFIED Requirements

### Requirement: Workspace Provisioning surfaces provider rate limiting and request rejection distinctly

Workspace Provisioning SHALL preserve distinct provider rate-limited and provider request-rejected failures when provider registry calls fail during provisioning, while separating failed sync attempts from durable Workspace failure state.

#### Scenario: Provider rate limiting blocks provisioning

- **WHEN** a provisioning sync encounters provider rate limiting and Native has not learned new authoritative terminal Workspace state
- **THEN** the Native Layer SHALL reject the sync with a retryable UI-safe `provider_rate_limited` command error
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the Native Layer MUST NOT mark the Workspace `ready`
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because of provider rate limiting
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider request rejection blocks provisioning

- **WHEN** a provisioning sync encounters provider request rejection and Native can safely preserve current Workspace metadata for user correction or later retry
- **THEN** the Native Layer SHALL reject the sync with a non-retryable UI-safe `provider_request_rejected` command error
- **AND** the recovery action SHALL guide the Client to reselect placement when applicable
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because the provider rejected the request
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider API failure reveals unsafe continuation

- **WHEN** a provider API failure occurs after a provider mutation or observation leaves Native unable to identify one safe continuation path from durable Workspace metadata and discoverable provider state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured provisioning failure detail for the failed phase and provider source
- **AND** the Native Layer SHALL retain existing Provider Resource snapshots and any newly known cleanup metadata
- **AND** the Native Layer MUST NOT create duplicate Provider Resources to recover from the failed sync

#### Scenario: Existing provisioning metadata is preserved on provider command failure

- **WHEN** provider rate limiting or provider request rejection prevents a provisioning sync from completing without producing new authoritative local or provider observation
- **THEN** the Native Layer SHALL preserve existing Workspace metadata
- **AND** the Native Layer SHALL preserve existing Provider Resource snapshots
- **AND** the Native Layer SHALL return a UI-safe command error instead of mutating the Workspace lifecycle state
