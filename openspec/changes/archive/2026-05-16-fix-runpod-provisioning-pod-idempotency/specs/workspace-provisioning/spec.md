## MODIFIED Requirements

### Requirement: Provision Temporary RunPod Provisioning Pod

Workspace Provisioning SHALL create, adopt, observe, and terminate a temporary RunPod provisioning pod that mounts the Workspace network volume at `/workspace` and runs the Provisioner Worker image, without blindly creating duplicate provider pods when local state is missing or incomplete.

#### Scenario: Existing correlated provisioning pod is adopted before create

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot and no active Provisioning Pod snapshot
- **AND** provider discovery finds exactly one live RunPod pod correlated to the Workspace by stable Workspace-derived pod name and network volume id
- **THEN** the Native Layer SHALL persist that pod as the active Provisioning Pod snapshot before contacting the Provisioner Worker
- **AND** the Native Layer MUST NOT create another RunPod pod
- **AND** the persisted snapshot SHALL include the provider pod id, selected data center id, selected GPU id, provider resource status, and Provisioner Worker status URL

#### Scenario: Multiple correlated provisioning pods fail closed

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot and no active Provisioning Pod snapshot
- **AND** provider discovery finds more than one live RunPod pod correlated to the Workspace
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known Persistent Storage Volume metadata and any safely representable matching pod metadata needed for cleanup or inspection
- **AND** the Native Layer MUST NOT create another RunPod pod

#### Scenario: Provisioning pod is created

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, no active Provisioning Pod snapshot, and no existing live Workspace-correlated RunPod provisioning pod
- **THEN** the Native Layer SHALL generate and store a per-workspace Provisioner Worker bearer token in secure storage
- **AND** the Native Layer SHALL create a RunPod pod using the configured Provisioner Worker image, selected GPU, selected data center, network volume id, and mount path `/workspace`
- **AND** the Native Layer SHALL inject the bearer token into the pod environment only for the Provisioner Worker runtime
- **AND** the Native Layer SHALL persist the active Provisioning Pod snapshot after the provider resource id is known
- **AND** the Native Layer SHALL use request-derived selected data center and selected GPU values when RunPod does not echo those fields in the pod response
- **AND** the Native Layer SHALL derive the Provisioner Worker status URL from the RunPod HTTP proxy URL when the pod exposes an HTTP port
- **AND** the Native Layer MUST NOT require RunPod direct TCP `publicIp` or `portMappings` metadata for HTTP-exposed provisioning pods

#### Scenario: Provisioning pod create response has a pod id but incomplete HTTP metadata

- **WHEN** RunPod creates a provisioning pod and returns a pod id with HTTP port exposure but without direct TCP `publicIp` or `portMappings`
- **THEN** the Native Layer SHALL persist an active Provisioning Pod snapshot using the pod id and RunPod HTTP proxy status URL
- **AND** the Native Layer MUST NOT return `provider_response_invalid` solely because direct TCP metadata is missing
- **AND** later sync SHALL observe the persisted pod instead of creating another pod

#### Scenario: Provisioning pod is observed

- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot
- **THEN** the Native Layer SHALL observe the RunPod pod status before contacting the Provisioner Worker
- **AND** the Native Layer SHALL update the active Provisioning Pod snapshot from the provider observation
- **AND** the Native Layer SHALL preserve the existing Provisioner Worker status URL when a later provider observation omits direct connectivity metadata
- **AND** the Native Layer SHALL mark the Workspace `failed` if the pod is failed, terminated unexpectedly, or unreachable in a way that prevents safe continuation

#### Scenario: Provisioning pod is terminated after preparation

- **WHEN** the Provisioner Worker has reported terminal success and the prepared environment timestamp is durable
- **THEN** the Native Layer SHALL delete or terminate the RunPod provisioning pod
- **AND** the Native Layer SHALL move the terminal pod snapshot to the last Provisioning Pod snapshot
- **AND** the Native Layer SHALL clear the active Provisioning Pod snapshot after termination is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token after the pod is confirmed no longer needed
