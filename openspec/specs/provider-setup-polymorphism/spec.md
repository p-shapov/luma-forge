# provider-setup-polymorphism Specification

## Purpose
TBD - created by archiving change refactor-provider-setup-polymorphism. Update Purpose after archive.
## Requirements
### Requirement: Provider Setup uses service-level provider capability dispatch
Provider Setup SHALL select provider-specific setup behavior through a narrow Provider Setup capability selected by `GpuCloudProviderId`.

#### Scenario: RunPod setup capability is selected
- **WHEN** Provider Setup needs provider-specific behavior for `runpod`
- **THEN** the Provider Setup registry SHALL return the concrete RunPod Provider Setup capability
- **AND** Provider Setup orchestration SHALL NOT directly call a provider-specific client or match provider request/response DTOs outside the registry/capability layer

#### Scenario: Provider-specific behavior remains concrete
- **WHEN** the RunPod Provider Setup capability validates a Provider API Key
- **THEN** it SHALL use the concrete RunPod client
- **AND** RunPod request and response shapes MUST remain in RunPod-specific modules
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers

### Requirement: Provider Setup preserves shared lifecycle behavior
Provider Setup SHALL keep provider-neutral setup lifecycle rules in the shared Provider Setup service.

#### Scenario: Shared lifecycle remains provider-neutral
- **WHEN** Provider Setup creates, reads, finalizes, or deletes setup state
- **THEN** keyring existence checks, keyring mutation order, stored-key re-read, rollback, redacted setup derivation, and domain validation SHALL remain shared application behavior
- **AND** provider-specific capability implementations SHALL NOT own those shared lifecycle decisions

#### Scenario: Provider errors remain UI-safe
- **WHEN** a provider-specific setup capability fails
- **THEN** it SHALL return existing `ProviderSetupError` variants
- **AND** the error MUST NOT expose Provider API Keys, provider response bodies, or keyring internals

