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
- **THEN** it SHALL own the RunPod provisioning sequence and call Workspace Resources through explicit provider-neutral resource operations
- **AND** RunPod request and response shapes MUST remain in `provider/runpod`, `workspace_resources/providers/runpod`, or RunPod-specific Workspace Provisioning modules
- **AND** RunPod endpoint template behavior MUST remain hidden inside RunPod Workspace Resources as part of serverless endpoint resource handling
- **AND** the low-level RunPod client MUST NOT be made generic over GPU providers

### Requirement: Workspace Provisioning preserves shared lifecycle behavior
Workspace Provisioning SHALL keep provider-neutral lifecycle, command, state-machine, progress, cancellation, and error behavior in the shared Workspace Provisioning service and provider-specific Workspace Provisioning capabilities.

#### Scenario: Shared lifecycle remains provider-neutral
- **WHEN** Workspace Provisioning initiates, syncs, or cancels provisioning
- **THEN** command contracts, workspace lookup, coordinator locking, draft-to-provisioning transition, Provider API Key prerequisite lookup, UI-safe error semantics, progress/result construction, and secret isolation SHALL remain shared Workspace Provisioning behavior
- **AND** provider-specific capability implementations SHALL NOT expose provider secrets, bearer tokens, raw provider request bodies, or raw provider response bodies

#### Scenario: Provider-specific provisioning chooses resource operations
- **WHEN** a provider-specific Workspace Provisioning capability derives the next safe provisioning action
- **THEN** it SHALL call exactly the explicit Workspace Resources operation needed for that action
- **AND** Workspace Resources MUST NOT choose the next provisioning phase from Workspace state

#### Scenario: RunPod behavior is unchanged
- **WHEN** the selected provider is `runpod`
- **THEN** Workspace Provisioning SHALL preserve the existing RunPod provisioning sequence, cancellation behavior, durable state transitions, cleanup failure handling, progress derivation, and command responses
- **AND** those behaviors SHALL be owned by Workspace Provisioning rather than Workspace Resources
- **AND** RunPod provider-specific provisioning behavior SHALL create the temporary Provisioning Pod with CPU compute instead of the selected GPU

### Requirement: RunPod Provisioning Pod uses cheapest CPU compute
RunPod Workspace Provisioning SHALL create the temporary Provisioning Pod with the hardcoded cheapest accepted CPU policy while preserving selected data center and network volume placement.

#### Scenario: RunPod Provisioning Pod is created with CPU compute
- **WHEN** RunPod provider-specific provisioning creates a Provisioning Pod
- **THEN** the RunPod pod create request SHALL use `computeType` `CPU`
- **AND** the request SHALL use `cpuFlavorIds` containing only `cpu3g`
- **AND** the request SHALL use `cpuFlavorPriority` `availability`
- **AND** the request SHALL use `vcpuCount` `2`
- **AND** the request MUST NOT include the selected GPU as a required pod GPU type

#### Scenario: RunPod Provisioning Pod remains colocated with workspace storage
- **WHEN** RunPod provider-specific provisioning creates a CPU Provisioning Pod
- **THEN** the RunPod pod create request SHALL use the Workspace Placement Plan's selected data center
- **AND** the request SHALL attach the Workspace network volume
- **AND** the request SHALL use `/workspace` for the pod volume mount path

#### Scenario: RunPod Serverless Endpoint keeps selected GPU placement
- **WHEN** RunPod provider-specific provisioning creates the Serverless Endpoint after environment preparation
- **THEN** the RunPod endpoint create request SHALL continue to use the Workspace Placement Plan's selected GPU
- **AND** the Provisioning Pod CPU policy MUST NOT change endpoint GPU placement
