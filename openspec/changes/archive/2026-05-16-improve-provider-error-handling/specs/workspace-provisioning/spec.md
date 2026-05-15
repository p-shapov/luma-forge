## ADDED Requirements

### Requirement: Workspace Provisioning surfaces provider rate limiting and request rejection distinctly

Workspace Provisioning SHALL preserve distinct provider rate-limited and provider request-rejected failures when provider registry calls fail during provisioning.

#### Scenario: Provider rate limiting blocks provisioning

- **WHEN** a provisioning sync encounters provider rate limiting
- **THEN** the Native Layer SHALL reject the sync with a retryable UI-safe `provider_rate_limited` command error
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the Native Layer MUST NOT mark the Workspace `ready`
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider request rejection blocks provisioning

- **WHEN** a provisioning sync encounters provider request rejection
- **THEN** the Native Layer SHALL reject the sync with a non-retryable UI-safe `provider_request_rejected` command error
- **AND** the recovery action SHALL guide the Client to reselect placement when applicable
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the command error MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Existing provisioning metadata is preserved on provider failure

- **WHEN** provider rate limiting or provider request rejection prevents a provisioning sync from completing
- **THEN** the Native Layer SHALL preserve existing Workspace metadata unless it has a new authoritative local or provider observation
- **AND** this change SHALL NOT introduce new Workspace lifecycle transitions for request rejection, missing resources, or indeterminate provider mutations
