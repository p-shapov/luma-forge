# workspace-provisioning-provider-polymorphism Specification

## Purpose
Define the service-level provider capability boundary used by Workspace Provisioning so provider-specific provisioning and cancellation behavior can vary by GPU Cloud Provider without making low-level provider clients generic.

## Requirements
### Requirement: Workspace Provisioning uses service-level provider capability dispatch
Workspace Provisioning SHALL select provider-specific provisioning behavior through a narrow Workspace Provisioning provider capability selected by `GpuCloudProviderId`.

#### Scenario: RunPod provisioning capability is selected
- **WHEN** Workspace Provisioning needs provider-specific sync or cancellation behavior for `runpod`
- **THEN** the Workspace Provisioning provider registry SHALL return the concrete RunPod Workspace Provisioning provider capability
- **AND** shared Workspace Provisioning orchestration SHALL NOT directly call a provider-specific client or match provider request/response DTOs outside the registry/capability layer

#### Scenario: Provider-specific provisioning behavior remains concrete
- **WHEN** the RunPod Workspace Provisioning provider capability syncs or cancels provisioning
- **THEN** it SHALL use RunPod-specific provisioning/resource modules for provider-specific behavior
- **AND** RunPod request and response shapes MUST remain in `provider/runpod`, `workspace_resources/providers/runpod`, or RunPod-specific Workspace Provisioning modules
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers

### Requirement: Workspace Provisioning preserves shared lifecycle behavior
Workspace Provisioning SHALL keep provider-neutral lifecycle, command, and error behavior in the shared Workspace Provisioning service.

#### Scenario: Shared lifecycle remains provider-neutral
- **WHEN** Workspace Provisioning initiates, syncs, or cancels provisioning
- **THEN** command contracts, workspace lookup, coordinator locking, draft-to-provisioning transition, Provider API Key prerequisite lookup, UI-safe error semantics, progress/result construction, and secret isolation SHALL remain shared Workspace Provisioning behavior
- **AND** provider-specific capability implementations SHALL NOT expose provider secrets, bearer tokens, raw provider request bodies, or raw provider response bodies

#### Scenario: RunPod behavior is unchanged
- **WHEN** the selected provider is `runpod`
- **THEN** Workspace Provisioning SHALL preserve the existing RunPod provisioning sequence, cancellation behavior, durable state transitions, cleanup failure fallback, progress derivation, and command responses
