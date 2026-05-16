## MODIFIED Requirements

### Requirement: Provision Temporary RunPod Provisioning Pod
Workspace Provisioning SHALL create, adopt, observe, and terminate a temporary RunPod provisioning pod that mounts the Workspace network volume at `/workspace` and runs the Provisioner Worker image from the Workspace's resolved runtime contract implementation snapshot, without blindly creating duplicate provider pods when local state is missing or incomplete.

#### Scenario: Existing correlated provisioning pod is adopted before create
- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot and no active Provisioning Pod snapshot
- **AND** provider discovery finds exactly one live RunPod pod correlated to the Workspace by stable Workspace-derived pod name, network volume id, and expected immutable Provisioner Worker image ref from the Workspace's resolved runtime contract implementation snapshot
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
- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, no active Provisioning Pod snapshot, and no existing safe Workspace-correlated RunPod provisioning pod
- **THEN** the Native Layer SHALL generate and store a per-workspace Provisioner Worker bearer token in secure storage
- **AND** the Native Layer SHALL create a RunPod pod using the immutable Provisioner Worker image ref from the Workspace's resolved runtime contract implementation snapshot, selected GPU, selected data center, network volume id, and mount path `/workspace`
- **AND** the Native Layer SHALL inject the bearer token into the pod environment only for the Provisioner Worker runtime
- **AND** the Native Layer SHALL persist the active Provisioning Pod snapshot after the provider resource id is known
- **AND** the Native Layer SHALL use request-derived selected data center and selected GPU values when RunPod does not echo those fields in the pod response
- **AND** the Native Layer SHALL derive the Provisioner Worker status URL from the RunPod HTTP proxy URL when the pod exposes an HTTP port
- **AND** the Native Layer MUST NOT require RunPod direct TCP `publicIp` or `portMappings` metadata for HTTP-exposed provisioning pods

#### Scenario: Provisioning pod create result is indeterminate
- **WHEN** a RunPod provisioning pod creation request times out or returns an indeterminate response after the per-workspace Provisioner Worker bearer token is stored
- **THEN** the Native Layer SHALL discover live RunPod pods correlated to the Workspace by stable Workspace-derived pod name, network volume id, and expected immutable Provisioner Worker image ref before retrying creation
- **AND** the Native Layer SHALL persist the active Provisioning Pod snapshot when exactly one safe matching pod exists
- **AND** the Native Layer SHALL mark the Workspace `failed` when zero or multiple safe matching pods exist
- **AND** the Native Layer SHALL retain known Persistent Storage Volume metadata
- **AND** the Native Layer MUST NOT create another RunPod pod while the previous create outcome is unresolved

#### Scenario: Provisioning pod create response has a pod id but incomplete HTTP metadata
- **WHEN** RunPod creates a provisioning pod and returns a pod id with HTTP port exposure but without direct TCP `publicIp` or `portMappings`
- **THEN** the Native Layer SHALL persist an active Provisioning Pod snapshot using the pod id and RunPod HTTP proxy status URL
- **AND** the Native Layer MUST NOT return `provider_response_invalid` solely because direct TCP metadata is missing
- **AND** later sync SHALL observe the persisted pod instead of creating another pod

#### Scenario: Provisioning pod is observed
- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot
- **THEN** the Native Layer SHALL observe the RunPod pod status before contacting the Provisioner Worker
- **AND** the Native Layer SHALL update the active Provisioning Pod snapshot from the provider observation
- **AND** the Native Layer SHALL verify that any provider-visible image metadata still matches the immutable Provisioner Worker image ref from the Workspace's resolved runtime contract implementation snapshot before continuing
- **AND** the Native Layer SHALL preserve the existing Provisioner Worker status URL when a later provider observation omits direct connectivity metadata
- **AND** the Native Layer SHALL mark the Workspace `failed` if the pod is failed, terminated unexpectedly, missing, or unreachable in a way that prevents safe continuation

#### Scenario: Tracked provisioning pod is missing during refresh
- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot
- **AND** provider observation reports that the tracked provisioning pod no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `starting_provisioning_pod`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known volume and pod metadata for cleanup or inspection
- **AND** the Native Layer MUST NOT clear the active pod snapshot and create another provisioning pod automatically

#### Scenario: Provisioning pod is terminated after preparation
- **WHEN** the Provisioner Worker has reported terminal success and the prepared environment timestamp is durable
- **THEN** the Native Layer SHALL delete or terminate the RunPod provisioning pod
- **AND** the Native Layer SHALL move the terminal pod snapshot to the last Provisioning Pod snapshot
- **AND** the Native Layer SHALL clear the active Provisioning Pod snapshot after termination is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token after the pod is confirmed no longer needed

### Requirement: Drive Provisioner Worker Preparation
Workspace Provisioning SHALL start and observe the Provisioner Worker job using the selected Workflow Preset and a per-workspace bearer token, while treating worker startup lag behind a running Provisioning Pod as non-terminal progress.

#### Scenario: Provisioner Worker is not ready after pod starts
- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot whose provider status is `running`
- **AND** the Provisioner Worker status endpoint is temporarily unreachable, times out, or returns a retryable unavailable or non-worker proxy response while Native can safely continue with the same active pod
- **THEN** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress with status `running`
- **AND** the progress phase SHALL indicate that provisioning is still waiting for the worker or preparing the environment
- **AND** the Native Layer MUST NOT mark the Workspace `failed`
- **AND** the Native Layer MUST NOT surface a user-facing `provisioner_worker_unavailable` command error for normal worker readiness lag
- **AND** the Native Layer MUST NOT create another Provisioning Pod

#### Scenario: Provisioner Worker job starts
- **WHEN** the active Provisioning Pod is running and the Provisioner Worker is reachable and idle
- **THEN** the Native Layer SHALL call `POST /start` with the active Workspace identifier as the worker job correlation identifier and the selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request
- **AND** the Native Layer SHALL include the Workspace's resolved runtime contract implementation snapshot in the worker start request
- **AND** the Native Layer SHALL treat the worker's accepted start response as running environment materialization progress

#### Scenario: Provisioner Worker idle status is valid
- **WHEN** the Provisioner Worker reports `status` `idle` with no active phase
- **THEN** the Native Layer SHALL treat the response as a valid idle worker status
- **AND** the Native Layer SHALL attempt to start the worker job when the Workspace still requires environment preparation
- **AND** the Native Layer MUST NOT mark the Workspace `failed` solely because the idle worker response has a null phase

#### Scenario: Provisioner Worker progress is reported
- **WHEN** the Provisioner Worker reports `running` or `cancelling` status for the active Workspace job
- **THEN** the Native Layer SHALL derive Workspace Provisioning Progress from the worker status, phase, progress percentage, and UI-safe diagnostic metadata
- **AND** the Native Layer SHALL map worker-specific phase names into Workspace Provisioning phases without exposing worker implementation details as durable domain state
- **AND** the Native Layer MUST NOT persist worker progress as authoritative lifecycle state

#### Scenario: Provisioner Worker succeeds
- **WHEN** the Provisioner Worker reports terminal success for the active Workspace job
- **THEN** the Native Layer SHALL persist the environment prepared timestamp
- **AND** later readiness validation SHALL depend on the prepared environment metadata when that metadata is available through the mounted workspace
- **AND** a terminal success response with no active phase SHALL be treated as valid

#### Scenario: Provisioner Worker fails
- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup
- **AND** returned diagnostics SHALL be UI-safe and MUST NOT contain bearer tokens, Provider API Keys, raw command output, stack traces, or environment dumps
- **AND** the Native Layer SHALL preserve stable UI-safe worker error metadata when the worker provides it

#### Scenario: Provisioner Worker API contract error is classified distinctly
- **WHEN** the Provisioner Worker returns an authenticated JSON validation error, malformed worker JSON success payload, unsupported status, unsafe progress percentage, or otherwise unrecoverable API contract response
- **THEN** the Native Layer SHALL classify the failure as a worker response or request contract problem
- **AND** the Native Layer MUST NOT classify that worker JSON response as worker unavailability
- **AND** temporary non-JSON proxy or readiness responses before the worker API is ready SHALL be treated as worker readiness lag rather than worker API contract failures
- **AND** any persisted or returned diagnostics SHALL remain UI-safe and secret-safe

### Requirement: Provision RunPod Serverless Template
Workspace Provisioning SHALL create, discover, adopt, or observe one per-user RunPod serverless template for the Endpoint Worker image from the Workspace's resolved runtime contract implementation snapshot and persist its provider-specific template id before creating the Serverless Endpoint, without blindly creating duplicate templates when create results are indeterminate.

#### Scenario: Serverless template is created
- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, no RunPod endpoint template snapshot, and no safe Workspace-correlated template exists
- **THEN** the Native Layer SHALL create a RunPod serverless template in the user's RunPod account using the immutable Endpoint Worker image ref from the Workspace's resolved runtime contract implementation snapshot
- **AND** the template SHALL use mount path `/workspace`
- **AND** the Native Layer SHALL persist the RunPod `template_id`, image reference, mount path, and provider status before creating an endpoint from that template

#### Scenario: Existing correlated template is adopted before create
- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, and no RunPod endpoint template snapshot
- **AND** provider discovery finds exactly one RunPod serverless template correlated to the Workspace by stable Workspace-derived template name and expected template properties
- **THEN** the Native Layer SHALL persist that template as the RunPod endpoint template snapshot
- **AND** the Native Layer MUST NOT create another RunPod serverless template

#### Scenario: Template snapshot is observed
- **WHEN** a provisioning Workspace already has a RunPod endpoint template snapshot
- **THEN** the Native Layer SHALL observe or validate that template before creating or validating the Serverless Endpoint
- **AND** the Native Layer MUST NOT blindly create a second template

#### Scenario: Template creation result is indeterminate
- **WHEN** a RunPod serverless template creation request times out or returns an indeterminate response
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discoverable provider resources before retrying creation
- **AND** the Native Layer SHALL adopt exactly one safe Workspace-correlated template when provider discovery proves that one exists
- **AND** the Native Layer SHALL mark the Workspace `failed` when it cannot identify exactly one safe Workspace-correlated template
- **AND** known volume metadata SHALL be retained for cleanup
- **AND** the Native Layer MUST NOT retry template creation while the previous create outcome is unresolved

#### Scenario: Template creation fails after provider success
- **WHEN** RunPod creates a serverless template but a later provider action fails
- **THEN** the Native Layer SHALL retain the persisted template snapshot
- **AND** future cleanup SHALL have enough metadata to delete the template even though Workspace Resource Cleanup is out of scope for this change

#### Scenario: Tracked template is missing during refresh
- **WHEN** a provisioning Workspace has a RunPod endpoint template snapshot
- **AND** provider observation reports that the tracked template no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `creating_endpoint_template`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known volume and template metadata for cleanup or inspection
- **AND** the Native Layer MUST NOT clear the template snapshot and create another template automatically

### Requirement: Provision RunPod Serverless Endpoint
Workspace Provisioning SHALL create, discover, adopt, or observe one RunPod Serverless Endpoint from the persisted per-user template and attach the Workspace network volume, without blindly creating duplicate endpoints when create results are indeterminate.

#### Scenario: Serverless endpoint is created
- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, a persisted RunPod endpoint template snapshot, no Serverless Endpoint snapshot, and no safe Workspace-correlated endpoint exists
- **THEN** the Native Layer SHALL create a RunPod Serverless Endpoint using the persisted `template_id`, selected GPU, selected data center, network volume id, and Endpoint Worker runtime values from the Workspace's resolved runtime contract implementation snapshot
- **AND** the endpoint SHALL use `workersMin = 0`, `workersMax = 1`, `scalerType = "REQUEST_COUNT"`, and `scalerValue = 1`
- **AND** the endpoint `idleTimeout` SHALL be set from the persisted RunPod Placement Plan endpoint keep-alive seconds
- **AND** the Native Layer SHALL persist the endpoint provider resource id, data center id, selected GPU id, provider status, and endpoint invoke URL after the provider resource id is known

#### Scenario: Existing correlated endpoint is adopted before create
- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, a persisted RunPod endpoint template snapshot, and no Serverless Endpoint snapshot
- **AND** provider discovery finds exactly one RunPod Serverless Endpoint correlated to the Workspace by stable Workspace-derived endpoint name, template id, network volume id, selected GPU, and selected data center
- **THEN** the Native Layer SHALL persist that endpoint as the Serverless Endpoint snapshot
- **AND** the Native Layer MUST NOT create another RunPod Serverless Endpoint

#### Scenario: Existing endpoint snapshot is refreshed
- **WHEN** a provisioning Workspace already has a Serverless Endpoint snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod Serverless Endpoint before readiness validation
- **AND** the Native Layer SHALL update the endpoint snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second endpoint

#### Scenario: Endpoint creation result is indeterminate
- **WHEN** a RunPod Serverless Endpoint creation request times out or returns an indeterminate response
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discoverable provider resources before retrying creation
- **AND** the Native Layer SHALL adopt exactly one safe Workspace-correlated endpoint when provider discovery proves that one exists
- **AND** the Native Layer SHALL mark the Workspace `failed` when it cannot identify exactly one safe Workspace-correlated endpoint
- **AND** known volume and template metadata SHALL be retained for cleanup
- **AND** the Native Layer MUST NOT retry endpoint creation while the previous create outcome is unresolved

#### Scenario: Endpoint setup fails
- **WHEN** endpoint creation, endpoint observation, or endpoint metadata validation fails in a way that prevents safe continuation
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for future cleanup

#### Scenario: Tracked endpoint is missing during refresh
- **WHEN** a provisioning Workspace has a Serverless Endpoint snapshot
- **AND** provider observation reports that the tracked endpoint no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `creating_endpoint`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for cleanup or inspection
- **AND** the Native Layer MUST NOT clear the endpoint snapshot and create another endpoint automatically

## ADDED Requirements

### Requirement: Adopt only runtime-compatible provisioning pods
Workspace Provisioning SHALL consider a discovered provisioning pod safe to adopt only when provider-visible metadata proves that it belongs to the Workspace and uses the expected runtime implementation image.

#### Scenario: Discovered provisioning pod matches expected runtime image
- **WHEN** provider discovery reports a live RunPod pod with the stable Workspace-derived pod name, the Workspace network volume id, and the immutable Provisioner Worker image ref from the Workspace's resolved runtime contract implementation snapshot
- **THEN** Workspace Provisioning MAY treat that pod as a safe matching pod for adoption when all other provider safety checks pass

#### Scenario: Discovered provisioning pod has a different runtime image
- **WHEN** provider discovery reports a live RunPod pod with the stable Workspace-derived pod name and Workspace network volume id but a different Provisioner Worker image ref than the Workspace's resolved runtime contract implementation snapshot
- **THEN** Workspace Provisioning SHALL treat that pod as unsafe to adopt
- **AND** it SHALL mark the Workspace `failed` with UI-safe provider resource mismatch detail rather than contacting that Provisioner Worker
- **AND** it MUST NOT create a replacement provisioning pod while the mismatched pod is live and correlated to the Workspace

#### Scenario: Provider cannot prove provisioning pod runtime image
- **WHEN** provider discovery cannot report enough image metadata to prove that a live correlated provisioning pod uses the expected immutable Provisioner Worker image ref
- **THEN** Workspace Provisioning SHALL fail closed with UI-safe provider metadata detail
- **AND** it MUST NOT adopt the pod, contact the Provisioner Worker, or create a duplicate provisioning pod

### Requirement: Keep base runtime dependency installation out of Workspace Provisioning
Workspace Provisioning SHALL treat base Python/PyTorch/ComfyUI runtime dependency installation as a Docker image build concern, not a provisioning concern.

#### Scenario: Provisioning prepares environment
- **WHEN** Workspace Provisioning drives the Provisioner Worker for a Workspace
- **THEN** the resulting environment preparation SHALL consist of extracting the baked base runtime archive, installing or verifying Workflow Preset Custom Nodes, validating the prepared environment, and downloading or verifying workspace assets
- **AND** Workspace Provisioning MUST NOT request or depend on provisioning-time base runtime dependency installation

#### Scenario: Selected GPU is used for provider resources
- **WHEN** Workspace Provisioning creates or observes RunPod compute resources for a selected GPU
- **THEN** the selected GPU SHALL determine provider resource placement
- **AND** the selected GPU MUST NOT determine which base runtime or Custom Node Python dependencies are installed

### Requirement: Resolve worker images from Workspace runtime implementation
Workspace Provisioning SHALL use the Workspace's persisted resolved runtime contract implementation snapshot as the source of worker image refs.

#### Scenario: Provisioning creates worker resources
- **WHEN** Workspace Provisioning creates a provisioning pod or endpoint template
- **THEN** it SHALL use the immutable provisioner and endpoint image refs from the Workspace's resolved runtime contract implementation snapshot
- **AND** it MUST NOT use global build-time worker image refs when a resolved runtime contract implementation snapshot is present

#### Scenario: Workspace runtime snapshot is missing
- **WHEN** Workspace Provisioning starts for a Workspace whose selected Workflow Preset requires a runtime contract but whose resolved runtime contract implementation snapshot is missing or invalid
- **THEN** the Native Layer SHALL reject or fail provisioning with a UI-safe readiness or metadata error
- **AND** it MUST NOT create provider resources with guessed worker image refs
