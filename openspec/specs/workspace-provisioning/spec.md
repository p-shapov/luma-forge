# Workspace Provisioning Specification

## Purpose
Define Native-owned Workspace Provisioning orchestration, progress, and worker interaction rules.
## Requirements
### Requirement: Drive Provisioner Worker Preparation
Workspace Provisioning SHALL start and observe the Provisioner Worker job using a worker-specific start request derived from the selected Workflow Preset's declared model assets and a per-workspace bearer token, while treating worker startup lag behind a running Provisioning Pod as non-terminal `starting_provisioning_pod` progress.

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
- **THEN** the Native Layer SHALL call `POST /start` with the active Workspace identifier as the worker job correlation identifier and a worker-specific model asset preparation payload derived from the selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request
- **AND** the Native Layer MUST NOT include Workflow Preset id, Workflow Preset version, workflow execution type, required base volume size, runtime contract reference, provisioner contract reference, the Workspace's resolved runtime image snapshot, resolved provisioner image snapshot, endpoint image fields, runtime manifest paths, or endpoint runtime paths in the worker start request body
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

#### Scenario: Selected GPU is used for runtime endpoint placement
- **WHEN** Workspace Provisioning creates or observes RunPod compute resources for a selected GPU
- **THEN** the selected GPU SHALL determine persistent runtime endpoint placement
- **AND** the selected GPU MUST NOT determine Provisioning Pod compute selection
- **AND** the selected GPU MUST NOT determine which base runtime dependencies are installed

#### Scenario: Provisioning Pod uses temporary provider compute
- **WHEN** Workspace Provisioning creates a Provisioning Pod for environment preparation
- **THEN** the Provisioning Pod SHALL run the Provisioner Worker on temporary provider-side compute attached to the Workspace network volume
- **AND** Workspace Provisioning MUST NOT require the Provisioning Pod to use GPU compute

### Requirement: Use resolved provisioner image snapshot
Workspace Provisioning SHALL use the Workspace's persisted resolved provisioner image snapshot as the authoritative source for Provisioner Worker image and workspace volume mount path.

#### Scenario: Provisioning creates provider resources
- **WHEN** Workspace Provisioning creates or reconciles provider resources for a Workspace
- **THEN** the Native Layer SHALL use `resolved_provisioner_image.provisioner_worker_image_ref` as the Provisioner Worker image ref
- **AND** the Native Layer SHALL use `resolved_provisioner_image.volume_mount_path` as the workspace volume mount path
- **AND** the Native Layer MUST NOT use NativeAppState constants or unversioned app-level provisioning defaults for those values

#### Scenario: Existing provider resources are checked for compatibility
- **WHEN** Workspace Provisioning observes a provider resource whose image ref or mount path is relevant to readiness or reconciliation
- **THEN** the Native Layer SHALL compare the observed provider resource data against the Workspace's persisted resolved runtime and provisioner snapshots
- **AND** the Native Layer SHALL treat mismatched provider resource metadata as not satisfying the Workspace's desired provisioning state

### Requirement: Pass resolved mount path to worker containers
Workspace Provisioning SHALL pass the resolved workspace volume mount path to worker containers as both provider mount configuration and worker process environment configuration.

#### Scenario: Provisioner Worker pod is created
- **WHEN** the Native Layer creates a Provisioner Worker pod
- **THEN** the RunPod pod volume mount path SHALL equal `resolved_provisioner_image.volume_mount_path`
- **AND** the pod environment SHALL include `LUMA_FORGE_WORKSPACE_MOUNT_PATH` with the same value

#### Scenario: Endpoint Worker template is created
- **WHEN** the Native Layer creates an Endpoint Worker template
- **THEN** the RunPod template volume mount path SHALL equal `resolved_provisioner_image.volume_mount_path`
- **AND** the template environment SHALL include `LUMA_FORGE_WORKSPACE_MOUNT_PATH` with the same value

### Requirement: Own resource operation sequencing
Workspace Provisioning SHALL own the provisioning state machine and decide which explicit Workspace Resources operation is safe to run for each sync iteration.

#### Scenario: Provisioning selects volume operation
- **WHEN** a provisioning Workspace requires persistent storage work
- **THEN** Workspace Provisioning SHALL decide whether to call Workspace Resources to create or observe the persistent storage volume
- **AND** Workspace Resources MUST NOT infer that decision from provisioning lifecycle state

#### Scenario: Provisioning selects provisioning pod operation
- **WHEN** a provisioning Workspace requires temporary provisioning compute work
- **THEN** Workspace Provisioning SHALL decide whether to call Workspace Resources to create, observe, or delete the provisioning pod
- **AND** Workspace Resources MUST NOT infer that decision from environment preparation state or provisioning lifecycle state

#### Scenario: Provisioning selects endpoint operation
- **WHEN** a provisioning Workspace requires serverless endpoint work
- **THEN** Workspace Provisioning SHALL decide whether to call Workspace Resources to create or observe the serverless endpoint
- **AND** Workspace Resources MUST NOT infer that decision from environment preparation state, active pod state, or provisioning lifecycle state

### Requirement: Own lifecycle and failure persistence
Workspace Provisioning SHALL be the only Workspace Provisioning module that sets provisioning lifecycle state, provisioning progress, provisioning phase, recovery action, or `last_provisioning_failure`.

#### Scenario: Resource operation succeeds
- **WHEN** a Workspace Resources operation succeeds and returns updated Workspace snapshots
- **THEN** Workspace Provisioning SHALL derive the next progress result from the updated Workspace
- **AND** Workspace Resources MUST NOT write provisioning progress or lifecycle state

#### Scenario: Resource operation fails
- **WHEN** a Workspace Resources operation returns a `WorkspaceResourceError`
- **THEN** Workspace Provisioning SHALL map the error using the current provisioning phase and recovery semantics
- **AND** Workspace Provisioning SHALL decide whether to return a command error, persist a provisioning failure, or continue with non-terminal progress
- **AND** Workspace Resources MUST NOT persist `last_provisioning_failure`

#### Scenario: Cancellation cleanup succeeds
- **WHEN** Workspace Provisioning cancels provisioning and Workspace Resources cleanup succeeds
- **THEN** Workspace Provisioning SHALL set the Workspace lifecycle state to `Draft`
- **AND** Workspace Provisioning SHALL clear provisioning failure state as needed for a clean draft Workspace

#### Scenario: Cancellation cleanup fails
- **WHEN** Workspace Provisioning cancels provisioning and Workspace Resources cleanup fails
- **THEN** Workspace Provisioning SHALL set the Workspace lifecycle state to `Failed`
- **AND** Workspace Provisioning SHALL persist a cancellation cleanup failure
- **AND** Workspace Resources MUST NOT set the Workspace lifecycle state to `Failed`

### Requirement: Serverless endpoint provider metadata is part of endpoint state
Workspace Provisioning SHALL treat optional serverless endpoint provider metadata as provider-specific endpoint management data, not generic provisioning state.

#### Scenario: RunPod endpoint template id is persisted with endpoint
- **WHEN** RunPod Workspace Resources creates a serverless endpoint
- **THEN** the resulting serverless endpoint snapshot SHALL include provider metadata containing the RunPod endpoint template identifier
- **AND** Workspace Provisioning SHALL use that endpoint snapshot as the authoritative cleanup metadata for the RunPod endpoint resource set

#### Scenario: Provider provisioning snapshot is not required
- **WHEN** Workspace Provisioning syncs or cancels a Workspace
- **THEN** it SHALL NOT require `provider_provisioning_snapshot` to track RunPod endpoint template cleanup metadata
- **AND** endpoint provider metadata SHALL replace that provider provisioning snapshot role

### Requirement: Enforce Hugging Face API key prerequisite before authenticated workflow provisioning
Workspace Provisioning SHALL detect when a selected Workflow Preset requires a Hugging Face API key and fail the Workspace before creating a Provisioner Pod when no configured Hugging Face API key exists.

#### Scenario: Required Hugging Face API key is missing
- **WHEN** Workspace Provisioning is about to create a Provisioner Pod for a Workspace whose selected Workflow Preset has `requires_hugging_face_api_key` set to `true`
- **AND** no Hugging Face API key exists in secure keyring storage
- **THEN** Workspace Provisioning SHALL mark the Workspace `Failed`
- **AND** it SHALL persist a structured provisioning failure whose recovery action directs the Client to configure Hugging Face setup before retry
- **AND** it MUST NOT create a Provisioner Pod
- **AND** it MUST NOT create or mutate provider resources solely to discover that the key is missing

#### Scenario: Required Hugging Face API key exists
- **WHEN** Workspace Provisioning is about to create a Provisioner Pod for a Workspace whose selected Workflow Preset has `requires_hugging_face_api_key` set to `true`
- **AND** a Hugging Face API key exists in secure keyring storage
- **THEN** Workspace Provisioning SHALL allow Provisioner Pod creation to proceed through the normal resource operation sequence
- **AND** it MUST NOT include the raw Hugging Face API key in Workspace metadata, progress, command responses, command errors, or logs

#### Scenario: Selected workflow does not require a Hugging Face API key
- **WHEN** Workspace Provisioning is about to create a Provisioner Pod for a Workspace whose selected Workflow Preset has `requires_hugging_face_api_key` set to `false`
- **THEN** Workspace Provisioning SHALL NOT require Hugging Face setup
- **AND** public Hugging Face asset downloads SHALL remain eligible to proceed without a configured Hugging Face API key

