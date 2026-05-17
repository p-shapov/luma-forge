## MODIFIED Requirements

### Requirement: Resolve worker images from Workspace runtime implementation
Workspace Provisioning SHALL use the Workspace's persisted resolved runtime image snapshot as the source of worker image refs.

#### Scenario: Provisioning creates worker resources
- **WHEN** Workspace Provisioning creates a provisioning pod or endpoint template
- **THEN** it SHALL use the immutable provisioner and endpoint image refs from the Workspace's resolved runtime image snapshot
- **AND** it MUST NOT use global build-time worker image refs when a resolved runtime image snapshot is present

#### Scenario: Workspace runtime snapshot is missing
- **WHEN** Workspace Provisioning starts for a Workspace whose selected Workflow Preset requires a runtime contract id/version pair but whose resolved runtime image snapshot is missing
- **THEN** the Native Layer SHALL reject or fail provisioning with a UI-safe readiness or metadata error
- **AND** it MUST NOT create provider resources with guessed worker image refs

### Requirement: Provisioning pod receives selected runtime image identity
Workspace Provisioning SHALL configure each temporary provisioning pod with the selected Provisioner Worker image ref and only operational environment values required by the Provisioner Worker.

#### Scenario: Provisioning pod is created with selected image
- **WHEN** the Native Layer creates a RunPod provisioning pod for a Workspace with a resolved runtime image snapshot
- **THEN** the pod image SHALL be the snapshot's provisioner image ref
- **AND** the pod environment SHALL include the unique `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`
- **AND** the pod environment MUST NOT include `LUMA_FORGE_PROVISIONER_IMAGE_REF`, runtime contract id, runtime contract version, implementation revision, runtime metadata, image metadata, registry credentials, provider API keys, or endpoint image identity

#### Scenario: Provisioner image ref is not available
- **WHEN** a Workspace lacks a resolved runtime image snapshot with a provisioner image ref
- **THEN** Workspace Provisioning SHALL fail before creating a RunPod provisioning pod
- **AND** it MUST NOT fall back to a build-time placeholder image ref

## REMOVED Requirements

### Requirement: RunPod pod observations use provider image field
**Reason**: Provisioning no longer validates provider pod image identity as runtime catalog metadata.
**Migration**: Native provisioning should create pods from the selected image ref and observe provider resource status without requiring pod image identity in the domain snapshot.
