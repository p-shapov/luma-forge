# workspace-resources-provider-polymorphism Specification

## Purpose
TBD - created by archiving change refactor-workspace-resources-provider-polymorphism. Update Purpose after archive.
## Requirements
### Requirement: Workspace Resources uses service-level provider capability dispatch
Workspace Resources SHALL select provider-specific resource behavior through a narrow Workspace Resources provider capability selected by `GpuCloudProviderId`.

#### Scenario: RunPod resource capability is selected
- **WHEN** Workspace Resources needs provider-specific behavior for `runpod`
- **THEN** the Workspace Resources provider registry SHALL return the concrete RunPod Workspace Resources provider capability
- **AND** shared Workspace Resources orchestration SHALL NOT directly call a provider-specific client or match provider request/response DTOs outside the registry/capability layer

#### Scenario: Provider-specific resource behavior remains concrete
- **WHEN** the RunPod Workspace Resources provider capability synchronizes or cleans up provider resources
- **THEN** it SHALL use the concrete RunPod client through RunPod-specific provider code
- **AND** RunPod request and response shapes MUST remain in `provider/runpod` or RunPod-specific Workspace Resources modules
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers

### Requirement: Workspace Resources preserves shared lifecycle behavior
Workspace Resources SHALL keep provider-neutral lifecycle rules in the shared Workspace Resources service.

#### Scenario: Shared lifecycle remains provider-neutral
- **WHEN** Workspace Resources synchronizes provider resources for Workspace Provisioning
- **THEN** workspace catalog persistence, UI-safe error semantics, command contracts, and secret isolation SHALL remain stable
- **AND** provider-specific capability implementations SHALL NOT expose provider secrets, raw provider request bodies, or raw provider response bodies

#### Scenario: Resource cleanup preserves shared reset semantics
- **WHEN** provider cleanup succeeds for known workspace resources
- **THEN** the shared Workspace Resources service SHALL reset provider resource state after provider cleanup
- **AND** the shared Workspace Resources service SHALL persist the reset workspace before reporting success
- **AND** the production RunPod cleanup behavior SHALL remain unchanged

