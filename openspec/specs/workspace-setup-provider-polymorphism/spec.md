# workspace-setup-provider-polymorphism Specification

## Purpose
TBD - created by archiving change refactor-workspace-setup-provider-polymorphism. Update Purpose after archive.
## Requirements
### Requirement: Workspace Setup uses service-level provider capability dispatch
Workspace Setup SHALL select provider-specific placement option behavior through a narrow Workspace Setup provider capability selected by `GpuCloudProviderId`.

#### Scenario: RunPod placement capability is selected
- **WHEN** Workspace Setup needs provider-specific placement option behavior for `runpod`
- **THEN** the Workspace Setup provider registry SHALL return the concrete RunPod Workspace Setup provider capability
- **AND** shared Workspace Setup orchestration SHALL NOT directly call a provider-specific client or match provider request/response DTOs outside the registry/capability layer

#### Scenario: Provider-specific placement behavior remains concrete
- **WHEN** the RunPod Workspace Setup provider capability fetches provider placement options
- **THEN** it SHALL use the concrete RunPod client
- **AND** RunPod request and response shapes MUST remain in RunPod-specific modules
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers

### Requirement: Workspace Setup preserves shared placement lifecycle behavior
Workspace Setup SHALL keep provider-neutral placement option lifecycle rules in the shared Workspace Setup service.

#### Scenario: Shared placement lifecycle remains provider-neutral
- **WHEN** Workspace Setup reads provider placement options
- **THEN** stored Provider API Key lookup, missing setup rejection, returned Provider Inventory validation, command contract mapping, and UI-safe response construction SHALL remain shared Workspace Setup behavior
- **AND** provider-specific capability implementations SHALL NOT own those shared lifecycle decisions

#### Scenario: Provider errors remain UI-safe
- **WHEN** a provider-specific Workspace Setup capability fails
- **THEN** it SHALL return existing `WorkspaceSetupError` variants
- **AND** the error MUST NOT expose Provider API Keys, provider response bodies, raw provider request bodies, or keyring internals

