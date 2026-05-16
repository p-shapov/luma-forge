## ADDED Requirements

### Requirement: Provisioning pod receives selected runtime image identity
Workspace Provisioning SHALL configure each temporary provisioning pod with both the selected Provisioner Worker image ref and a non-secret environment value declaring that same immutable image identity.

#### Scenario: Provisioning pod is created with selected image identity
- **WHEN** the Native Layer creates a RunPod provisioning pod for a Workspace with a resolved runtime implementation snapshot
- **THEN** the pod image SHALL be the snapshot's provisioner image ref
- **AND** the pod environment SHALL include `LUMA_FORGE_PROVISIONER_IMAGE_REF` with that same ref
- **AND** the bearer token SHALL remain the only secret injected into the pod environment

#### Scenario: Provisioner image identity is not available
- **WHEN** a Workspace lacks a valid resolved runtime implementation provisioner image ref
- **THEN** Workspace Provisioning SHALL fail before creating a RunPod provisioning pod
- **AND** it MUST NOT fall back to a build-time placeholder image ref

### Requirement: RunPod pod observations use provider image field
Workspace Provisioning SHALL parse RunPod pod image identity from the provider response shape when mapping provisioning pod observations.

#### Scenario: Pod response contains image field
- **WHEN** RunPod returns a pod response with a non-empty `image` value
- **THEN** the Native Layer SHALL map that value as the pod image identity
- **AND** native provider discovery SHALL NOT filter otherwise-owned provisioning pods by image identity

#### Scenario: Pod image identity is missing
- **WHEN** RunPod returns a pod response without a usable image identity
- **THEN** the Native Layer SHALL treat the pod response as invalid provider data
