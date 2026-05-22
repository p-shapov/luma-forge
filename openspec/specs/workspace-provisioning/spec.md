# Workspace Provisioning Specification

## Purpose
Define Native-owned Workspace Provisioning orchestration, progress, and worker interaction rules.

## Requirements
### Requirement: Drive Provisioner Worker Preparation
Workspace Provisioning SHALL start and observe the Provisioner Worker job using the selected Workflow Preset and a per-workspace bearer token, while treating worker startup lag behind a running Provisioning Pod as non-terminal `starting_provisioning_pod` progress.

#### Scenario: Provisioner Worker is not ready after pod starts
- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot whose provider status is `running`
- **AND** the Provisioner Worker status endpoint is temporarily unreachable, times out, or returns a retryable unavailable or non-worker proxy response while Native can safely continue with the same active pod
- **THEN** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress with status `running`
- **AND** the progress phase SHALL be `starting_provisioning_pod`
- **AND** the progress percent SHALL be `10`
- **AND** the Native Layer MUST NOT mark the Workspace `failed`
- **AND** the Native Layer MUST NOT surface a user-facing `provisioner_worker_unavailable` command error for normal worker readiness lag
- **AND** the Native Layer MUST NOT create another Provisioning Pod

#### Scenario: Provisioner Worker job starts
- **WHEN** the active Provisioning Pod is running and the Provisioner Worker is reachable and idle
- **THEN** the Native Layer SHALL call `POST /start` with the active Workspace identifier as the worker job correlation identifier and the selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request
- **AND** the Native Layer MUST NOT include the Workspace's resolved runtime image snapshot, endpoint image fields, runtime manifest paths, or endpoint runtime paths in the worker start request
- **AND** the Native Layer SHALL treat a worker start response that has not yet reported active preparation work as `starting_provisioning_pod` progress
- **AND** the progress percent SHALL be `10`

#### Scenario: Provisioner Worker idle status is valid
- **WHEN** the Provisioner Worker reports `status` `idle` with no active phase
- **THEN** the Native Layer SHALL treat the response as a valid idle worker status
- **AND** the Native Layer SHALL attempt to start the worker job when the Workspace still requires environment preparation
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because the idle worker response has a null phase

#### Scenario: Provisioner Worker progress is reported
- **WHEN** the Provisioner Worker reports `running` or `cancelling` status for the active Workspace job
- **THEN** the Native Layer SHALL derive Workspace Provisioning Progress from the worker status, phase, and progress percentage
- **AND** the Native Layer SHALL map worker-specific phase names into Workspace Provisioning phases without exposing worker implementation details as durable domain state
- **AND** the Native Layer SHALL scale worker-local preparation percentages into total Workspace Provisioning percentages
- **AND** the Native Layer MUST NOT persist worker progress as authoritative lifecycle state

### Requirement: Keep base runtime dependency installation out of Workspace Provisioning
Workspace Provisioning SHALL treat endpoint Python, PyTorch, ComfyUI, runtime extensions, runtime extension Python dependencies, runtime manifests, model validation, and output validation as Endpoint Worker image or generation concerns, not provisioning concerns.

#### Scenario: Provisioning prepares environment
- **WHEN** Workspace Provisioning drives the Provisioner Worker for a Workspace
- **THEN** the resulting environment preparation SHALL consist of preparing workspace directories, downloading or verifying declared model assets, and validating the declared model assets exist
- **AND** Workspace Provisioning MUST NOT request or depend on provisioning-time endpoint runtime validation, workflow validation, output directory validation, runtime manifest writing, ComfyUI dependency installation, runtime extension checkout, runtime extension dependency installation, Python overlay creation, or pip execution
- **AND** Workspace Provisioning MUST NOT require the provisioner image to match the selected endpoint runtime image

#### Scenario: Selected GPU is used for provider resources
- **WHEN** Workspace Provisioning creates or observes RunPod compute resources for a selected GPU
- **THEN** the selected GPU SHALL determine provider resource placement
- **AND** the selected GPU MUST NOT determine which base runtime dependencies are installed
