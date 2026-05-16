## ADDED Requirements

### Requirement: Native command logs include only stable UI-safe command metadata

Native command logging SHALL keep provider-related failures observable through stable command metadata without exposing provider transport details or secrets.

#### Scenario: Command fails due to provider error

- **WHEN** a Tauri command fails because a provider error was mapped into a native command error
- **THEN** the command failure log MAY include the provider id, command error code, retryability, field, reason, and recovery action
- **AND** the log MUST NOT include Provider API Keys, bearer headers, raw provider request bodies, raw provider response bodies, provider-specific error codes, stack traces, keyring details, worker bearer tokens, or raw provider error text

#### Scenario: Provider request rejection is logged

- **WHEN** a provider request rejection reaches the native command logging boundary
- **THEN** the log SHALL include only stable UI-safe command metadata such as `provider_request_rejected`, retryability, reason, and recovery action
- **AND** the log MUST NOT include RunPod-specific rejection messages, raw response payloads, placement request bodies, or secrets
