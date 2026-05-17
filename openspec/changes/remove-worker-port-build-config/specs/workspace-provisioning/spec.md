## ADDED Requirements

### Requirement: Use provider-owned RunPod worker port values
Workspace Provisioning SHALL use fixed provider/worker implementation values for RunPod worker port exposure instead of reading worker ports from native build-time configuration.

#### Scenario: RunPod provisioning pod is created
- **WHEN** Workspace Provisioning creates a temporary RunPod provisioning pod
- **THEN** the Native Layer SHALL expose the Provisioner Worker HTTP port from provider/provisioning implementation code
- **AND** it MUST NOT read `LUMA_FORGE_PROVISIONER_WORKER_PORT` from Cargo build environment output, root `.env`, or runtime application configuration

#### Scenario: RunPod serverless template is created
- **WHEN** Workspace Provisioning creates a RunPod serverless template and RunPod requires a container port declaration for the endpoint container
- **THEN** the Native Layer SHALL use a provider/provisioning implementation value named for the endpoint container's internal ComfyUI HTTP port
- **AND** it MUST NOT model that value as a generic Endpoint Worker API port
- **AND** it MUST NOT read `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT` from Cargo build environment output, root `.env`, or runtime application configuration

#### Scenario: Worker image refs are selected
- **WHEN** Workspace Provisioning creates a provisioning pod or endpoint template
- **THEN** worker image refs SHALL still come from the Workspace's resolved runtime contract implementation snapshot
- **AND** fixed provider/worker port values MUST NOT replace or weaken Runtime Catalog ownership of worker image identity
