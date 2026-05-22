# workspace-provisioning Specification

## Purpose
Define the native-owned Workspace Provisioning lifecycle for transitioning a saved Draft Workspace into a Ready Workspace by creating provider resources, preparing the remote runtime environment, synchronizing progress, handling cancellation, and preserving cleanup metadata on failure.
## Requirements
### Requirement: Initiate Workspace Provisioning

The Native Layer SHALL expose a Workspace Provisioning initiation command that validates one saved `Draft` Workspace and transitions it to `Provisioning` before provider mutation work continues through sync.

#### Scenario: Draft workspace enters provisioning

- **WHEN** the Client initiates provisioning for a saved `Draft` Workspace whose Placement Plan is complete and whose Provider API Key exists in secure storage
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `provisioning`
- **AND** the Native Layer SHALL return the authoritative Workspace metadata and Workspace Provisioning Progress with status `running`
- **AND** the Native Layer MUST NOT create Provider Resources before the lifecycle transition is durable

#### Scenario: Non-draft workspace is rejected

- **WHEN** the Client initiates provisioning for a Workspace whose lifecycle state is not `draft`
- **THEN** the Native Layer SHALL reject the request with a UI-safe provisioning error
- **AND** the Native Layer MUST NOT create, modify, or delete Provider Resources

#### Scenario: Provider setup prerequisite is missing

- **WHEN** the Client initiates provisioning and the required Provider API Key is missing or unreadable from secure storage
- **THEN** the Native Layer SHALL reject the request before provider mutation
- **AND** the Native Layer MUST NOT persist a provisioning lifecycle transition

### Requirement: Sync Workspace Provisioning

The Native Layer SHALL expose a Workspace Provisioning sync command that derives the next safe activity from durable Workspace state and performs at most one provider, worker, or catalog mutation per sync call.

#### Scenario: Sync performs one safe action

- **WHEN** the Client syncs a Workspace whose lifecycle state is `provisioning`
- **THEN** the Native Layer SHALL read the authoritative Workspace metadata before selecting work
- **AND** the Native Layer SHALL perform at most one safe provisioning activity
- **AND** the Native Layer SHALL persist any resulting Workspace metadata before reporting success for that activity
- **AND** the Native Layer SHALL return authoritative Workspace metadata and derived Workspace Provisioning Progress

#### Scenario: Concurrent sync is read-only

- **WHEN** a provisioning sync is already active for the same Workspace
- **THEN** another sync request for that Workspace SHALL return the latest persisted Workspace metadata and derived Progress
- **AND** the concurrent sync MUST NOT perform duplicate provider, worker, or catalog mutation work

#### Scenario: Sync for non-provisioning workspace is idle

- **WHEN** the Client syncs a Workspace whose lifecycle state is `draft`, `ready`, or `failed`
- **THEN** the Native Layer SHALL return the authoritative Workspace metadata and derived terminal or idle Progress
- **AND** the Native Layer MUST NOT create Provider Resources

### Requirement: Provision RunPod Network Volume

Workspace Provisioning SHALL create, detect orphaned owned resources for, or observe one RunPod network volume for the Workspace and persist its snapshot before later compute resources depend on it, without adopting pre-existing provider volumes when local Workspace metadata has no matching snapshot.

#### Scenario: Network volume is created

- **WHEN** a provisioning Workspace has no Persistent Storage Volume snapshot
- **AND** provider discovery finds no RunPod network volume with the stable Workspace-derived volume name
- **THEN** the Native Layer SHALL create a RunPod network volume in the selected data center with the requested size
- **AND** the Native Layer SHALL persist the provider resource id, data center id, provisioned size, provider status, and mount path `/workspace`
- **AND** the Native Layer SHALL use the Workspace identifier as the stable correlation value when provider naming supports it

#### Scenario: Existing volume snapshot is refreshed

- **WHEN** a provisioning Workspace already has a Persistent Storage Volume snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod network volume before using it for later readiness decisions
- **AND** the Native Layer SHALL update the snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second network volume

#### Scenario: Same-name volume is orphaned before create

- **WHEN** a provisioning Workspace has no Persistent Storage Volume snapshot
- **AND** provider discovery finds one or more RunPod network volumes with the stable Workspace-derived volume name
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_orphaned_resources`, phase `creating_volume`, source `provider_resource`, and cleanup-oriented recovery action
- **AND** the Native Layer SHALL retain known cleanup metadata when safely representable
- **AND** the Native Layer MUST NOT adopt the discovered volume
- **AND** the Native Layer MUST NOT create another RunPod network volume

#### Scenario: Volume creation result is indeterminate

- **WHEN** a RunPod network volume creation request times out or returns an indeterminate response before a Persistent Storage Volume snapshot is durable
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discover provider resources by the stable Workspace-derived volume name before retrying creation
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_orphaned_resources` when discovery finds one or more same-name volumes
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_operation_indeterminate` when discovery finds no same-name volume
- **AND** known cleanup metadata SHALL be retained
- **AND** the Native Layer MUST NOT retry volume creation while the previous create outcome is unresolved

#### Scenario: Tracked volume is missing during refresh

- **WHEN** a provisioning Workspace has a Persistent Storage Volume snapshot
- **AND** provider observation reports that the tracked network volume no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `creating_volume`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known Workspace metadata and cleanup metadata
- **AND** the Native Layer MUST NOT clear the snapshot and create another network volume automatically

### Requirement: Provision Temporary RunPod Provisioning Pod

Workspace Provisioning SHALL create, detect orphaned owned resources for, observe, and terminate a temporary RunPod provisioning pod that mounts the Workspace network volume at `/workspace` and runs the Provisioner Worker image from app or provider deployment configuration, without adopting pre-existing provider pods when local Workspace metadata has no matching snapshot.

#### Scenario: Same-name provisioning pod is orphaned before create

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot and no active Provisioning Pod snapshot
- **AND** provider discovery finds one or more live RunPod pods with the stable Workspace-derived pod name
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_orphaned_resources`, phase `starting_provisioning_pod`, source `provider_resource`, and cleanup-oriented recovery action
- **AND** the Native Layer SHALL retain known Persistent Storage Volume metadata and any safely representable matching pod metadata needed for cleanup or inspection
- **AND** the Native Layer MUST NOT adopt the discovered provisioning pod
- **AND** the Native Layer MUST NOT create another RunPod pod

#### Scenario: Provisioning pod is created

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, no active Provisioning Pod snapshot, and no live RunPod pod with the stable Workspace-derived pod name
- **THEN** the Native Layer SHALL generate and store a per-workspace Provisioner Worker bearer token in secure storage
- **AND** the Native Layer SHALL create a RunPod pod using the immutable Provisioner Worker image ref from app or provider deployment configuration, selected GPU, selected data center, network volume id, and mount path `/workspace`
- **AND** the Native Layer SHALL inject the bearer token into the pod environment only for the Provisioner Worker runtime
- **AND** the Native Layer SHALL persist the active Provisioning Pod snapshot after the provider resource id is known
- **AND** the Native Layer SHALL use request-derived selected data center and selected GPU values when RunPod does not echo those fields in the pod response
- **AND** the Native Layer SHALL derive the Provisioner Worker status URL from the RunPod HTTP proxy URL when the pod exposes an HTTP port
- **AND** the Native Layer MUST NOT require RunPod direct TCP `publicIp` or `portMappings` metadata for HTTP-exposed provisioning pods
- **AND** Workspace Provisioning MUST NOT require the provisioner image to match the selected endpoint runtime image

#### Scenario: Provisioning pod create result is indeterminate

- **WHEN** a RunPod provisioning pod creation request times out or returns an indeterminate response after the per-workspace Provisioner Worker bearer token is stored and before an active Provisioning Pod snapshot is durable
- **THEN** the Native Layer SHALL discover live RunPod pods by the stable Workspace-derived pod name before retrying creation
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_orphaned_resources` when discovery finds one or more same-name pods
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_operation_indeterminate` when discovery finds no same-name pod
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
- **THEN** the provisioning pod image ref SHALL come from app or provider deployment configuration
- **AND** the endpoint template image ref SHALL come from the Workspace's resolved runtime image snapshot
- **AND** fixed provider/worker port values MUST NOT replace or weaken endpoint Runtime Catalog ownership

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
- **AND** the Native Layer MUST NOT include the Workspace's resolved runtime image snapshot or endpoint image fields in the worker start request
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

#### Scenario: Provisioner Worker succeeds
- **WHEN** the Provisioner Worker reports terminal success for the active Workspace job
- **THEN** the Native Layer SHALL persist the environment prepared timestamp
- **AND** later readiness validation SHALL depend on the prepared workspace metadata when that metadata is available through the mounted workspace
- **AND** a terminal success response with no active phase SHALL be treated as valid

#### Scenario: Provisioner Worker fails
- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL persist a `WorkspaceProvisioningFailure` with source `provisioner_worker`, phase `preparing_environment`, and a stable UI-safe worker failure code
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup
- **AND** returned error metadata SHALL be UI-safe and MUST NOT contain bearer tokens, Provider API Keys, raw command output, stack traces, or environment dumps
- **AND** the Native Layer SHALL preserve stable UI-safe worker error metadata when the worker provides it
- **AND** the Native Layer SHALL return authoritative Workspace metadata and Workspace Provisioning Progress instead of returning `Err(NativeCommandError)` when the failure has been persisted

#### Scenario: Granular Provisioner Worker preparation failure is persisted
- **WHEN** a provisioning sync observes terminal worker preparation failure cause `asset_download_failed`, `asset_auth_required`, `path_validation_failed`, `step_timeout`, or `unexpected_error`
- **THEN** the Native Layer SHALL persist the matching `WorkspaceProvisioningFailureCode`
- **AND** the persisted failure source SHALL be `provisioner_worker`
- **AND** the persisted failure phase SHALL be `preparing_environment`
- **AND** the persisted failure recovery action SHALL be `inspect_workspace_provisioning`
- **AND** the sync response SHALL include authoritative failed Workspace metadata and failed Workspace Provisioning Progress
- **AND** the sync response MUST NOT return a direct `NativeCommandErrorCode::ProvisionerWorkerFailed`

#### Scenario: Provisioner Worker API contract error is classified distinctly
- **WHEN** the Provisioner Worker returns an authenticated JSON validation error, malformed worker JSON success payload, unsupported status, unsafe progress percentage, or otherwise unrecoverable API contract response
- **THEN** the Native Layer SHALL classify the failure as a worker response or request contract problem
- **AND** the Native Layer MUST NOT classify that worker JSON response as worker unavailability
- **AND** temporary non-JSON proxy or readiness responses before the worker API is ready SHALL be treated as worker readiness lag rather than worker API contract failures
- **AND** any persisted or returned error metadata SHALL remain UI-safe and secret-safe

### Requirement: Own Provisioner Worker Integration Internally
Workspace Provisioning SHALL own Provisioner Worker gateway communication and environment preparation synchronization inside the `workspace_provisioning` native module.

#### Scenario: Worker gateway is internal to workspace provisioning
- **WHEN** the Native Layer starts or observes Provisioner Worker preparation during Workspace Provisioning
- **THEN** it SHALL use gateway types owned by `workspace_provisioning`
- **AND** it MUST NOT depend on a standalone `workspace_provisioner` crate module

#### Scenario: Worker behavior is preserved after module consolidation
- **WHEN** Workspace Provisioning communicates with the Provisioner Worker after the module consolidation
- **THEN** the request payloads, status parsing, progress derivation, persisted Workspace metadata updates, and UI-safe failure mapping SHALL remain behaviorally equivalent to the previous implementation
- **AND** secrets MUST remain confined to secure storage and provider or worker call paths

#### Scenario: Environment preparation remains part of provisioning orchestration
- **WHEN** a provisioning Workspace reaches the environment preparation step
- **THEN** Workspace Provisioning SHALL start or observe the Provisioner Worker job as part of its own sync orchestration
- **AND** it MUST preserve the existing rule that each sync call performs at most one provider, worker, or catalog mutation

### Requirement: Provision RunPod Serverless Endpoint

Workspace Provisioning SHALL create, detect orphaned owned resources for, or observe one RunPod Serverless Endpoint and the RunPod endpoint template needed to create it, without exposing endpoint template handling as a separate Workspace Provisioning phase or adopting pre-existing provider resources when local Workspace metadata has no matching snapshot.

#### Scenario: Serverless template is created as endpoint setup

- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, no RunPod endpoint template snapshot, and no RunPod serverless template with the stable Workspace-derived template name
- **THEN** the Native Layer SHALL create a RunPod serverless template in the user's RunPod account using the immutable Endpoint Worker image ref from the Workspace's resolved runtime image snapshot
- **AND** the template SHALL use mount path `/workspace`
- **AND** the Native Layer SHALL persist the RunPod `template_id`, image reference, mount path, and provider status before creating an endpoint from that template
- **AND** Workspace Provisioning Progress SHALL continue to expose this work as phase `creating_endpoint`

#### Scenario: Same-name template is orphaned before endpoint setup can create it

- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, and no RunPod endpoint template snapshot
- **AND** provider discovery finds one or more RunPod serverless templates with the stable Workspace-derived template name
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_orphaned_resources`, phase `creating_endpoint`, source `provider_resource`, and cleanup-oriented recovery action
- **AND** the Native Layer SHALL retain known volume and template metadata for cleanup or inspection when safely representable
- **AND** the Native Layer MUST NOT adopt the discovered template
- **AND** the Native Layer MUST NOT create another RunPod serverless template

#### Scenario: Template snapshot is observed during endpoint setup

- **WHEN** a provisioning Workspace already has a RunPod endpoint template snapshot
- **THEN** the Native Layer SHALL observe or validate that template before creating or validating the Serverless Endpoint
- **AND** the Native Layer MUST NOT blindly create a second template
- **AND** Workspace Provisioning Progress SHALL expose this work as phase `creating_endpoint`

#### Scenario: Template creation result is indeterminate during endpoint setup

- **WHEN** a RunPod serverless template creation request times out or returns an indeterminate response before a RunPod endpoint template snapshot is durable
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discover provider resources by the stable Workspace-derived template name before retrying creation
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_orphaned_resources` and phase `creating_endpoint` when discovery finds one or more same-name templates
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_operation_indeterminate` and phase `creating_endpoint` when discovery finds no same-name template
- **AND** known volume metadata SHALL be retained for cleanup
- **AND** the Native Layer MUST NOT retry template creation while the previous create outcome is unresolved

#### Scenario: Template creation fails after provider success

- **WHEN** RunPod creates a serverless template but a later provider action fails
- **THEN** the Native Layer SHALL retain the persisted template snapshot
- **AND** future cleanup SHALL have enough metadata to delete the template even though Workspace Resource Cleanup is out of scope for this change

#### Scenario: Tracked template is missing during endpoint setup refresh

- **WHEN** a provisioning Workspace has a RunPod endpoint template snapshot
- **AND** provider observation reports that the tracked template no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `creating_endpoint`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known volume and template metadata for cleanup or inspection
- **AND** the Native Layer MUST NOT clear the template snapshot and create another template automatically

#### Scenario: Serverless endpoint is created

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, a persisted RunPod endpoint template snapshot, no Serverless Endpoint snapshot, and no RunPod Serverless Endpoint with the stable Workspace-derived endpoint name
- **THEN** the Native Layer SHALL create a RunPod Serverless Endpoint using the persisted `template_id`, selected GPU, selected data center, network volume id, and Endpoint Worker runtime values from the Workspace's resolved runtime image snapshot
- **AND** the endpoint SHALL use `workersMin = 0`, `workersMax = 1`, `scalerType = "REQUEST_COUNT"`, and `scalerValue = 1`
- **AND** the endpoint `idleTimeout` SHALL be set from the persisted RunPod Placement Plan endpoint keep-alive seconds
- **AND** the Native Layer SHALL persist the endpoint provider resource id, data center id, selected GPU id, provider status, and endpoint invoke URL after the provider resource id is known

#### Scenario: Same-name endpoint is orphaned before create

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, a persisted RunPod endpoint template snapshot, and no Serverless Endpoint snapshot
- **AND** provider discovery finds one or more RunPod Serverless Endpoints with the stable Workspace-derived endpoint name
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_orphaned_resources`, phase `creating_endpoint`, source `provider_resource`, and cleanup-oriented recovery action
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for cleanup or inspection when safely representable
- **AND** the Native Layer MUST NOT adopt the discovered endpoint
- **AND** the Native Layer MUST NOT create another RunPod Serverless Endpoint

#### Scenario: Existing endpoint snapshot is refreshed

- **WHEN** a provisioning Workspace already has a Serverless Endpoint snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod Serverless Endpoint before readiness validation
- **AND** the Native Layer SHALL update the endpoint snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second endpoint

#### Scenario: Endpoint creation result is indeterminate

- **WHEN** a RunPod Serverless Endpoint creation request times out or returns an indeterminate response before a Serverless Endpoint snapshot is durable
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discover provider resources by the stable Workspace-derived endpoint name before retrying creation
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_orphaned_resources` when discovery finds one or more same-name endpoints
- **AND** the Native Layer SHALL mark the Workspace `failed` with code `provider_operation_indeterminate` when discovery finds no same-name endpoint
- **AND** known volume and template metadata SHALL be retained for cleanup
- **AND** the Native Layer MUST NOT retry endpoint creation while the previous create outcome is unresolved

#### Scenario: Endpoint setup fails

- **WHEN** endpoint template creation, endpoint template observation, endpoint creation, endpoint observation, or endpoint metadata validation fails in a way that prevents safe continuation
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for future cleanup
- **AND** any persisted failure phase SHALL be `creating_endpoint`

#### Scenario: Tracked endpoint is missing during refresh

- **WHEN** a provisioning Workspace has a Serverless Endpoint snapshot
- **AND** provider observation reports that the tracked endpoint no longer exists
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured failure detail with code `provider_resource_missing`, phase `creating_endpoint`, and source `provider_resource`
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for cleanup or inspection
- **AND** the Native Layer MUST NOT clear the endpoint snapshot and create another endpoint automatically

### Requirement: Validate Provisioning Readiness

Workspace Provisioning SHALL mark a Workspace `ready` only after required provider resources and the prepared workspace environment are durably represented and provider observations confirm readiness.

#### Scenario: Workspace becomes ready

- **WHEN** the Persistent Storage Volume, RunPod endpoint template, and Serverless Endpoint snapshots are persisted and provider observations confirm the required resources still exist in acceptable states
- **AND** the prepared environment timestamp is durable
- **AND** prepared workspace metadata identifies the image-baked runtime contract and workspace-specific assets required by the Endpoint Worker
- **AND** no active Provisioning Pod snapshot remains
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `ready`
- **AND** Workspace Provisioning Progress SHALL report status `completed`

#### Scenario: Unknown resource status blocks readiness

- **WHEN** any required provider resource has status `unknown`, is missing, or cannot be confirmed by provider observation
- **THEN** the Native Layer MUST NOT mark the Workspace `ready`
- **AND** the Native Layer SHALL mark the Workspace `failed` when safe continuation or later validation is impossible

#### Scenario: Readiness excludes generation

- **WHEN** Workspace Provisioning validates the persistent runtime entry point
- **THEN** the Native Layer SHALL validate provider metadata, prepared workspace metadata, and no-job endpoint health or status information only
- **AND** the Native Layer MUST NOT submit a generation request to the Endpoint Worker

### Requirement: Cancel Workspace Provisioning

Workspace Provisioning SHALL support user cancellation while a Workspace is in `provisioning` by using shared known-resource cleanup behavior and returning the Workspace to `draft` only after cancellation cleanup succeeds.

#### Scenario: Cancellation succeeds

- **WHEN** the Client cancels provisioning for a Workspace in `provisioning`
- **AND** no sync or cancellation operation already owns the same Workspace
- **THEN** the Native Layer SHALL invoke shared cleanup behavior for the Workspace-owned Provider Resources known from authoritative Workspace metadata
- **AND** shared cleanup SHALL delete the Serverless Endpoint, RunPod endpoint template, Provisioning Pod, and Persistent Storage Volume resources known from Workspace metadata when they exist
- **AND** shared cleanup MUST NOT call the Provisioner Worker `/cancel` endpoint during destructive Workspace Provisioning cancellation
- **AND** the Native Layer SHALL tolerate already-missing provider resources
- **AND** the Native Layer SHALL clear provisioning snapshots and return the Workspace lifecycle state to `draft` only after cleanup is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token when it exists

#### Scenario: Cancellation conflicts with active sync

- **WHEN** the Client cancels provisioning for a Workspace while another sync or cancellation operation already owns that Workspace
- **THEN** the Native Layer SHALL reject the cancellation with a retryable UI-safe conflict error
- **AND** the Native Layer MUST NOT return unchanged provisioning metadata as a successful cancellation
- **AND** the Native Layer MUST NOT clear snapshots, delete Provider Resources, or delete local Provisioner Worker bearer tokens in the conflicting cancellation request

#### Scenario: Cancellation skips worker cancel even when worker metadata exists

- **WHEN** the Client cancels provisioning for a Workspace with an active Provisioning Pod snapshot and Provisioner Worker token
- **AND** the active Provisioning Pod snapshot contains a Provisioner Worker status URL
- **AND** Native deletes or confirms missing all known Provider Resources
- **AND** Native deletes the stored Provisioner Worker bearer token
- **THEN** the Native Layer SHALL return the Workspace lifecycle state to `draft`
- **AND** the Native Layer MUST NOT call the Provisioner Worker `/cancel` endpoint
- **AND** the Native Layer MUST NOT persist `cancellation_cleanup_failed` for any worker cancellation outcome because worker cancellation is not part of destructive cancellation cleanup

#### Scenario: Cancellation cleanup is incomplete

- **WHEN** cancellation cannot confirm deletion of all known Provider Resources or required local Provisioner Worker bearer token cleanup
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain all known Provider Resource and RunPod template metadata for future Workspace Resource Cleanup

#### Scenario: Shared cleanup preserves policy-specific final mutation

- **WHEN** shared known-resource cleanup succeeds for a cancellation request
- **THEN** Workspace Provisioning SHALL apply the cancellation policy by clearing provisioning snapshots and returning the existing Workspace Catalog entry to `draft`
- **AND** shared cleanup behavior MUST NOT delete the Workspace Catalog entry during provisioning cancellation

### Requirement: Reuse Known Workspace Resource Cleanup

The Native Layer SHALL centralize deletion of known Workspace-owned provisioning resources and local provisioning credentials so Workspace Provisioning cancellation and future Workspace Resource Cleanup use the same provider deletion and token cleanup semantics.

#### Scenario: Known resources are cleaned in dependency-safe order

- **WHEN** shared cleanup receives Workspace metadata with known provisioning resources
- **THEN** it SHALL attempt provider cleanup in dependency-safe order: Serverless Endpoint, RunPod endpoint template, active Provisioning Pod, and Persistent Storage Volume
- **AND** it MUST NOT attempt to cancel the active Provisioner Worker job during destructive Workspace Provisioning cancellation
- **AND** it SHALL attempt to delete the per-workspace Provisioner Worker bearer token for the Workspace regardless of whether active Provisioning Pod metadata exists
- **AND** it SHALL tolerate already-missing Provider Resources
- **AND** it SHALL report cleanup success only after all known Provider Resources are deleted or confirmed missing and required local cleanup succeeds

#### Scenario: Cleanup removes token without active pod snapshot

- **WHEN** shared cleanup receives Workspace metadata without an active Provisioning Pod snapshot
- **AND** a per-workspace Provisioner Worker bearer token exists in secure storage
- **THEN** shared cleanup SHALL delete the per-workspace Provisioner Worker bearer token
- **AND** shared cleanup SHALL tolerate an already-missing token only when the secret store reports successful deletion or absence without exposing secret values
- **AND** shared cleanup MUST NOT require active pod metadata to perform local token cleanup

#### Scenario: Cleanup final state is chosen by caller policy

- **WHEN** shared cleanup returns a result to a caller
- **THEN** the caller SHALL decide the final local Workspace Catalog mutation
- **AND** Workspace Provisioning cancellation SHALL return the Workspace to `draft` on cleanup success
- **AND** future Workspace Resource Cleanup MAY delete the Workspace Catalog entry on cleanup success
- **AND** cleanup failure SHALL preserve known metadata for later recovery

### Requirement: Preserve Secret Safety During Provisioning

Workspace Provisioning SHALL keep Provider API Keys and Provisioner Worker bearer tokens out of Workspace metadata, command responses, logs, error metadata, and generated frontend bindings.

#### Scenario: Provider API Key is used

- **WHEN** Workspace Provisioning calls RunPod
- **THEN** the Native Layer SHALL read the Provider API Key from secure storage through the provider registry
- **AND** the Provider API Key MUST NOT be written to Workspace Catalog metadata, command responses, logs, error metadata, or generated frontend types

#### Scenario: Provisioner bearer token is stored

- **WHEN** Workspace Provisioning creates a temporary Provisioning Pod
- **THEN** the Native Layer SHALL store the Provisioner Worker bearer token in a per-workspace keyring entry separate from Provider API Key storage
- **AND** the bearer token MUST NOT be written to Workspace Catalog metadata, command responses, logs, error metadata, or generated frontend types

#### Scenario: Provisioner bearer token is removed

- **WHEN** the active Provisioning Pod is confirmed terminated, successful cancellation returns the Workspace to `draft`, or future cleanup removes the Workspace
- **THEN** the Native Layer SHALL delete the per-workspace Provisioner Worker bearer token when it exists
- **AND** missing token entries SHALL be tolerated after no active provisioning pod remains

### Requirement: Workspace Provisioning surfaces provider rate limiting and request rejection distinctly

Workspace Provisioning SHALL preserve distinct provider rate-limited and provider request-rejected failures when provider registry calls fail during provisioning, and SHALL record them as durable Workspace failure state when they block active provisioning progress.

#### Scenario: Provider rate limiting blocks provisioning

- **WHEN** a provisioning sync encounters provider rate limiting and Native has not learned new authoritative terminal Workspace state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured provisioning failure detail with provider-rate-limited code, provider source, failed phase, and recovery action
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the Native Layer MUST NOT mark the Workspace `ready`
- **AND** the failure detail MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider request rejection blocks provisioning

- **WHEN** a provisioning sync encounters provider request rejection and Native can safely preserve current Workspace metadata for user correction or later retry
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured provisioning failure detail with provider-request-rejected code, provider source, failed phase, and recovery action
- **AND** the recovery action SHALL guide the Client to reselect placement when applicable
- **AND** the Native Layer MUST NOT clear existing Provider Resource snapshots
- **AND** the failure detail MUST NOT expose provider-specific error codes or raw provider response details

#### Scenario: Provider API failure reveals unsafe continuation

- **WHEN** a provider API failure occurs after a provider mutation or observation leaves Native unable to identify one safe continuation path from durable Workspace metadata and discoverable provider state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist structured provisioning failure detail for the failed phase and provider source
- **AND** the Native Layer SHALL retain existing Provider Resource snapshots and any newly known cleanup metadata
- **AND** the Native Layer MUST NOT create duplicate Provider Resources to recover from the failed sync

#### Scenario: Existing provider metadata is preserved on blocking provider failure

- **WHEN** provider rate limiting or provider request rejection prevents a provisioning sync from completing without producing new authoritative local or provider observation
- **THEN** the Native Layer SHALL preserve existing Provider Resource snapshots and cleanup metadata
- **AND** the Native Layer SHALL persist structured provisioning failure detail instead of returning a retryable sync command error

### Requirement: Record structured provisioning failure details

Workspace Provisioning SHALL persist a structured, UI-safe provisioning failure detail whenever it persists a Workspace lifecycle state of `failed`.

#### Scenario: Terminal provider resource failure is recorded

- **WHEN** a provisioning sync observes a required provider resource in a terminal failed, unexpectedly terminated, unknown, missing, orphaned, or otherwise unsafe-to-continue state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail with a stable failure code, failed phase, provider-resource source, retryability, and recovery action
- **AND** the Native Layer SHALL retain known Provider Resource snapshots for future cleanup

#### Scenario: Terminal worker failure is recorded

- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error during provisioning
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail with a stable failure code, failed phase, provisioner-worker source, retryability, and recovery action
- **AND** the Native Layer SHALL include stable UI-safe worker error code or reason metadata when provided by the worker contract
- **AND** the Native Layer SHALL preserve stable typed worker failure codes for both terminal worker job failures and unrecoverable worker API contract failures when the worker provides them
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup

#### Scenario: Unsafe continuation is recorded

- **WHEN** a provider mutation outcome, readiness validation result, local token inconsistency, discovered orphaned provider resource, or cleanup result leaves Native unable to safely continue provisioning without risking duplicate resources, leaked resources, or a false `ready` state
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `failed`
- **AND** the Native Layer SHALL persist a structured provisioning failure detail describing the failed phase, failure source, and recovery action
- **AND** the Native Layer SHALL retain all known cleanup metadata

#### Scenario: Failed progress includes failure detail

- **WHEN** the Client initiates, syncs, cancels, or reads a Workspace whose lifecycle state is `failed` and whose metadata contains structured provisioning failure detail
- **THEN** the Native Layer SHALL return Workspace Provisioning Progress with status `failed`
- **AND** the returned progress or Workspace payload SHALL expose the structured failure detail through generated binding-safe types
- **AND** React SHALL NOT need to parse a free-form message string to classify the failure

#### Scenario: Legacy failed workspace has no failure detail

- **WHEN** the Client reads or syncs a Workspace whose lifecycle state is `failed` but whose persisted metadata predates structured provisioning failure detail
- **THEN** the Native Layer SHALL return failed progress with a generic UI-safe failure classification
- **AND** the Native Layer MUST NOT infer provider-specific detail that is not present in durable metadata

#### Scenario: Failure details are secret-safe

- **WHEN** the Native Layer records or returns structured provisioning failure detail
- **THEN** the failure detail MUST NOT include Provider API Keys, Provisioner Worker bearer tokens, raw provider responses, provider-specific secret-bearing URLs, raw command output, stack traces, environment dumps, worker request bodies, or raw worker responses

### Requirement: Preserve Workspace Provisioning Behavior During Native Refactor
The Native Layer SHALL preserve the existing Workspace Provisioning command contract, durable sync semantics, cleanup metadata guarantees, and secret-safety behavior when the native provisioning implementation is split into focused Rust modules.

#### Scenario: Refactored sync preserves single-action semantics
- **WHEN** the Client syncs a Workspace whose lifecycle state is `provisioning`
- **THEN** the Native Layer SHALL continue to derive the next safe provisioning activity from authoritative Workspace metadata
- **AND** the Native Layer SHALL perform at most one provider, worker, or catalog mutation activity for that sync call
- **AND** the Native Layer SHALL persist resulting Workspace metadata before reporting success for the activity

#### Scenario: Refactored module preserves command contract
- **WHEN** the Client initiates, syncs, or cancels Workspace Provisioning through existing Tauri commands
- **THEN** the Native Layer SHALL return the same UI-safe response shapes and error categories as before the refactor
- **AND** generated frontend bindings MUST NOT require a frontend import or contract change because of the native module split

#### Scenario: Refactored implementation preserves cleanup metadata
- **WHEN** a provisioning action fails after any provider resource identifier is known
- **THEN** the Native Layer SHALL retain the known Workspace resource snapshots required for future cleanup
- **AND** the Native Layer MUST NOT clear provisioning snapshots except through the existing successful cancellation cleanup policy

#### Scenario: Refactored implementation preserves secret safety
- **WHEN** Workspace Provisioning reads Provider API Keys or Provisioner Worker bearer tokens during initiate, sync, or cancel
- **THEN** the Native Layer SHALL keep those secrets behind secure storage and provider or worker call paths
- **AND** command responses, Workspace metadata, logs, error metadata, and generated frontend bindings MUST NOT expose those secrets

### Requirement: Route provider-specific provisioning choreography through use-case steps
Workspace Provisioning SHALL keep provider-specific provisioning sequence decisions inside provider-specific Workspace Provisioning step modules while preserving provider API access behind infrastructure gateways.

#### Scenario: RunPod sync is dispatched at the Workspace Provisioning boundary
- **WHEN** the Client syncs a RunPod Workspace whose lifecycle state is `provisioning`
- **THEN** the Native Layer SHALL route the provisioning sequence through RunPod-specific Workspace Provisioning step code
- **AND** the Native Layer SHALL continue to derive each safe action from authoritative Workspace metadata
- **AND** the Native Layer SHALL continue to perform at most one provider, worker, or catalog mutation per sync call

#### Scenario: Provider gateway remains infrastructure
- **WHEN** RunPod-specific Workspace Provisioning step code needs to create, observe, discover, or delete provider resources
- **THEN** it SHALL call provider resource gateway methods for low-level provider operations
- **AND** it MUST NOT move Workspace lifecycle, progress, idempotency, or cleanup metadata decisions into the low-level provider client or provider registry

#### Scenario: RunPod endpoint template is handled as endpoint implementation detail
- **WHEN** a provisioning RunPod Workspace has a prepared environment and no active Provisioning Pod
- **THEN** the RunPod Workspace Provisioning endpoint step SHALL manage both the RunPod serverless template and the RunPod Serverless Endpoint sequencing needed for the persistent runtime entry point
- **AND** it SHALL preserve the existing RunPod endpoint template snapshot metadata required for resume, readiness, and cleanup
- **AND** it SHALL preserve existing Serverless Endpoint snapshot behavior and failure classifications

#### Scenario: Refactor preserves external contract
- **WHEN** Workspace Provisioning provider step modules are refactored
- **THEN** existing Tauri command request and response shapes SHALL remain compatible
- **AND** generated frontend bindings MUST NOT require frontend changes because of the native module split
- **AND** persisted Workspace Catalog metadata MUST remain readable without a migration for this change

### Requirement: Keep base runtime dependency installation out of Workspace Provisioning
Workspace Provisioning SHALL treat endpoint Python, PyTorch, ComfyUI, runtime extensions, and runtime extension Python dependencies as Endpoint Worker image build concerns or future runtime-image concerns, not provisioning concerns.

#### Scenario: Provisioning prepares environment
- **WHEN** Workspace Provisioning drives the Provisioner Worker for a Workspace
- **THEN** the resulting environment preparation SHALL consist of preparing workspace directories, downloading or verifying workspace model assets, writing prepared workspace metadata, and validating the prepared workspace
- **AND** Workspace Provisioning MUST NOT request or depend on provisioning-time endpoint runtime validation, ComfyUI dependency installation, runtime extension checkout, runtime extension dependency installation, Python overlay creation, or pip execution
- **AND** Workspace Provisioning MUST NOT require the provisioner image to match the selected endpoint runtime image

#### Scenario: Selected GPU is used for provider resources
- **WHEN** Workspace Provisioning creates or observes RunPod compute resources for a selected GPU
- **THEN** the selected GPU SHALL determine provider resource placement
- **AND** the selected GPU MUST NOT determine which base runtime dependencies are installed

### Requirement: Resolve worker images from separate deployment contracts
Workspace Provisioning SHALL use a generic Provisioner Worker image for workspace preparation and a workflow/runtime-specific Endpoint Worker image for generation.

#### Scenario: Provisioning creates worker resources
- **WHEN** Workspace Provisioning creates a provisioning pod or endpoint template
- **THEN** it SHALL use the immutable provisioner image ref from app or provider deployment configuration for the provisioning pod
- **AND** it SHALL use the immutable endpoint image ref from the Workspace's resolved runtime image snapshot for the endpoint template

#### Scenario: Workspace runtime snapshot is missing
- **WHEN** Workspace Provisioning starts for a Workspace whose selected Workflow Preset requires a runtime contract id/version pair but whose resolved runtime image snapshot is missing
- **THEN** the Native Layer SHALL reject or fail endpoint-template provisioning with a UI-safe readiness or metadata error
- **AND** it MUST NOT create endpoint provider resources with guessed endpoint image refs

### Requirement: Provisioning pod receives generic provisioner image identity
Workspace Provisioning SHALL configure each temporary provisioning pod with the configured Provisioner Worker image ref and only operational environment values required by the Provisioner Worker.

#### Scenario: Provisioning pod is created with configured image
- **WHEN** the Native Layer creates a RunPod provisioning pod for a Workspace
- **THEN** the pod image SHALL be the configured provisioner image ref
- **AND** the pod environment SHALL include the unique `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`
- **AND** the pod environment MUST NOT include `LUMA_FORGE_PROVISIONER_IMAGE_REF`, runtime contract id, runtime contract version, implementation revision, runtime metadata, image metadata, registry credentials, provider API keys, or endpoint image identity

#### Scenario: Provisioner image ref is not configured
- **WHEN** app or provider deployment configuration lacks a provisioner image ref
- **THEN** Workspace Provisioning SHALL fail before creating a RunPod provisioning pod
- **AND** it MUST NOT fall back to a build-time placeholder image ref

### Requirement: Exclude RunPod Template Runtime Environment From Workspace Metadata
Workspace Provisioning SHALL treat RunPod serverless template runtime environment values as transient provider observation data and MUST NOT persist or return those values as Workspace metadata.

#### Scenario: Template is created from provider observation
- **WHEN** Workspace Provisioning creates a RunPod serverless template and receives a provider observation that includes runtime environment values
- **THEN** the Native Layer SHALL persist the RunPod endpoint template snapshot without runtime environment keys or values
- **AND** the persisted snapshot SHALL retain the template id, endpoint worker image reference, mount path, and provider resource status needed for later provisioning and cleanup

#### Scenario: Existing template snapshot is refreshed
- **WHEN** Workspace Provisioning observes or validates a persisted RunPod endpoint template before creating or validating a Serverless Endpoint
- **AND** the provider observation includes runtime environment values
- **THEN** the Native Layer SHALL update only UI-safe template snapshot metadata
- **AND** the Native Layer MUST NOT add runtime environment keys or values to the Workspace metadata

#### Scenario: Legacy template metadata contains runtime environment
- **WHEN** Workspace Provisioning reads a Workspace whose existing RunPod endpoint template metadata contains a legacy runtime environment map
- **THEN** the Native Layer SHALL tolerate the legacy field for compatibility
- **AND** any subsequent persisted Workspace snapshot SHALL omit the legacy runtime environment map
- **AND** provisioning continuation SHALL use only safe template metadata for reuse, endpoint creation, readiness validation, and cleanup

### Requirement: Manage Workspace Resources Through Workspace Resource Operations

Workspace Provisioning SHALL delegate Workspace-owned provider resource lifecycle to a concrete `workspace_resources` native service that selects provider-specific resource implementation from the Workspace provider identity while preserving existing provisioning behavior, command contracts, persisted Workspace metadata, and generated frontend bindings.

#### Scenario: Provisioning delegates resource lifecycle

- **WHEN** Workspace Provisioning needs to create, observe, discover, delete, write snapshots for, or clear snapshots for Workspace-owned provider resources
- **THEN** the Native Layer SHALL route that resource lifecycle work through `workspace_resources`
- **AND** Workspace Provisioning SHALL remain responsible for initiation, lifecycle gating, concurrency guarding, phase ordering, Provisioner Worker coordination, and result shaping
- **AND** Workspace Provisioning MUST NOT directly own provider resource snapshot write or clear logic after the resource operation boundary is introduced

#### Scenario: Resource service dispatches by Workspace provider identity

- **WHEN** `workspace_resources` performs Workspace-owned provider resource lifecycle work for a Workspace
- **THEN** the Native Layer SHALL select the provider-specific resource implementation by matching the Workspace's persisted GPU Cloud Provider id
- **AND** the Native Layer MUST NOT select the provider-specific resource implementation through a production alias that hardcodes RunPod before a Workspace is known
- **AND** the Native Layer MUST NOT require a top-level `WorkspaceResourceOperations` trait to route Workspace Resource lifecycle calls

#### Scenario: Resource operation persists matching snapshot state

- **WHEN** a Workspace resource operation creates, observes, deletes, or confirms absence of a Workspace-owned provider resource
- **THEN** the operation SHALL persist the matching Workspace metadata mutation before reporting success for that resource activity
- **AND** the operation SHALL preserve known cleanup metadata when provider mutation, observation, or validation fails after a provider resource identifier is known
- **AND** the operation SHALL return authoritative Workspace metadata for provisioning result derivation

#### Scenario: Resource operations preserve one-action sync semantics

- **WHEN** the Client syncs a Workspace whose lifecycle state is `provisioning`
- **THEN** Workspace Provisioning and `workspace_resources` together SHALL perform at most one provider, worker, or catalog mutation activity for that sync call
- **AND** moving resource lifecycle into `workspace_resources` MUST NOT allow a single sync call to create, delete, or persist multiple provisioning resources beyond the behavior already required by Workspace Provisioning

#### Scenario: RunPod operations use low-level RunPod client

- **WHEN** `workspace_resources` performs RunPod resource lifecycle work
- **THEN** it SHALL keep raw RunPod HTTP request/response mapping inside `provider::runpod`
- **AND** it SHALL keep Provider API Keys behind secure storage and provider-call paths
- **AND** it MUST NOT expose Provider API Keys, Provisioner Worker bearer tokens, raw provider responses, or secret-bearing details through Workspace metadata, command responses, logs, or generated frontend bindings

#### Scenario: RunPod endpoint template remains internal to endpoint operation

- **WHEN** `workspace_resources` manages a RunPod Serverless Endpoint for a Workspace
- **THEN** the RunPod endpoint operation SHALL manage the RunPod endpoint template as an internal implementation detail needed for Serverless Endpoint creation
- **AND** the Workspace resource operation facade MUST NOT expose endpoint template as a separate top-level Workspace resource operation
- **AND** the Native Layer SHALL continue to persist existing RunPod endpoint template snapshot metadata required for resume, readiness validation, and cleanup

#### Scenario: Provider resources boundary is removed

- **WHEN** Workspace resource lifecycle operations are refactored
- **THEN** the Native Layer SHALL remove the generic `provider_resources` provider-resource gateway boundary from the provisioning path
- **AND** provider setup and placement concerns MAY remain in provider registry code
- **AND** raw RunPod API integration SHALL remain in `provider::runpod`
- **AND** frontend command request and response shapes MUST remain compatible

#### Scenario: Cleanup uses shared Workspace Resource lifecycle

- **WHEN** Workspace Provisioning cancellation or future Workspace Resource Cleanup deletes known Workspace-owned resources
- **THEN** the Native Layer SHALL use shared `workspace_resources` cleanup behavior
- **AND** cleanup SHALL preserve the existing dependency-safe deletion order for RunPod resources
- **AND** cleanup SHALL tolerate already-missing Provider Resources
- **AND** cleanup SHALL delete the per-workspace Provisioner Worker bearer token when it exists without exposing secret values

### Requirement: Workspace Provisioning synchronizes provider resources through Workspace Resources
Workspace Provisioning SHALL use Workspace Resources to synchronize provider-owned resources, and Workspace Resources SHALL select provider-specific behavior through a service-level provider capability.

#### Scenario: RunPod provisioning resource sync remains unchanged
- **WHEN** Workspace Provisioning synchronizes a RunPod workspace's network volume, provisioning pod, endpoint template, serverless endpoint, or known-resource cleanup
- **THEN** Workspace Resources SHALL select the concrete RunPod Workspace Resources provider capability through centralized `GpuCloudProviderId` dispatch
- **AND** RunPod resource creation, discovery, observation, cleanup, error mapping, persistence, and failure semantics SHALL remain unchanged
- **AND** Workspace Provisioning command contracts and user-facing behavior SHALL remain unchanged

### Requirement: Select provider-specific Workspace Provisioning behavior through provider capability
Workspace Provisioning SHALL centralize provider-specific provisioning flow selection behind a Workspace Provisioning provider capability selected by `GpuCloudProviderId`.

#### Scenario: Shared sync delegates provider-specific flow
- **WHEN** a provisioning Workspace is synced after shared lifecycle and concurrency checks pass
- **THEN** the shared Workspace Provisioning service SHALL select the Workspace Provisioning provider capability for the Workspace provider id
- **AND** provider-specific sync sequencing SHALL execute inside the selected provider capability
- **AND** the RunPod provider capability SHALL preserve the existing RunPod sequence and behavior

#### Scenario: Shared cancel delegates provider-specific cleanup
- **WHEN** a provisioning Workspace cancellation passes shared lifecycle and concurrency checks
- **THEN** the shared Workspace Provisioning service SHALL select the Workspace Provisioning provider capability for the Workspace provider id
- **AND** provider-specific cancellation cleanup SHALL execute inside the selected provider capability
- **AND** existing cleanup failure fallback semantics SHALL remain unchanged

### Requirement: Provisioning preserves Workspace Catalog error categories

Workspace Provisioning SHALL preserve Workspace Catalog error categories received from Workspace Setup, Workspace Catalog repository operations, or Workspace Resources instead of collapsing them into generic catalog unavailability.

#### Scenario: Initiate provisioning sees specific catalog error

- **WHEN** initiating Workspace Provisioning fails because loading or updating the Workspace Catalog returns storage unavailable, migration failed, query failed, corrupt data, or schema mismatch
- **THEN** Workspace Provisioning SHALL return the corresponding provisioning error category as an immediate command failure
- **AND** the Native Layer MUST NOT create, modify, or delete Provider Resources
- **AND** the Native Layer MUST NOT persist a new Workspace failure for the catalog failure

#### Scenario: Sync provisioning sees specific catalog error

- **WHEN** syncing Workspace Provisioning fails because loading or updating the Workspace Catalog returns storage unavailable, migration failed, query failed, corrupt data, or schema mismatch
- **THEN** Workspace Provisioning SHALL return the corresponding provisioning error category as an immediate command failure
- **AND** Workspace Provisioning MUST NOT hide the original catalog category behind generic Workspace Catalog unavailable behavior

### Requirement: Provisioning persists recovery-required resource failures

Workspace Provisioning SHALL persist `WorkspaceProvisioningFailure` records when resource-operation failures require user inspection, cleanup, or durable recovery state.

#### Scenario: Provider operation is indeterminate

- **WHEN** a provider resource create, observe, or cleanup operation is indeterminate and provider resource state may be unsafe
- **THEN** Workspace Provisioning SHALL persist a structured failure with provider-resource source and cleanup-oriented recovery action
- **AND** Workspace Provisioning MUST NOT return only a generic command error for the unsafe state

#### Scenario: Tracked provider resource is missing

- **WHEN** a tracked provider resource is missing during observation or cleanup
- **THEN** Workspace Provisioning SHALL persist a structured failure with provider-resource source and cleanup-oriented recovery action
- **AND** known Workspace metadata and cleanup metadata SHALL be retained

#### Scenario: Orphaned provider resources are discovered

- **WHEN** provider discovery finds Workspace-owned or same-name provider resources that cannot be safely adopted
- **THEN** Workspace Provisioning SHALL persist a structured failure with cleanup-oriented recovery action and stable UI-safe failure metadata
- **AND** Workspace Provisioning MUST NOT adopt the resource or create a duplicate resource

#### Scenario: Cancellation cleanup is incomplete

- **WHEN** cancellation cannot confirm deletion of all known provider resources or required local Provisioner Worker token cleanup
- **THEN** Workspace Provisioning SHALL persist a structured failure with cleanup-oriented recovery action
- **AND** Workspace Provisioning MUST NOT return the Workspace to `draft`

### Requirement: Provisioning handles provider and worker transient failures by phase

Workspace Provisioning SHALL return command errors, non-mutating progress, or persisted failures for provider and worker failures according to phase-specific recovery semantics.

#### Scenario: Provider API is unavailable or rate limited during provisioning

- **WHEN** provider API unavailability or rate limiting prevents an active provisioning sync from completing
- **THEN** Workspace Provisioning SHALL persist a structured provisioning failure with the appropriate provider availability or rate-limited code
- **AND** Workspace Provisioning SHALL transition the Workspace lifecycle state to `failed`
- **AND** the persisted failure recovery action SHALL be the durable recovery signal

#### Scenario: Provider request is rejected or response is invalid

- **WHEN** a provider request is rejected or a provider response is invalid
- **THEN** Workspace Provisioning SHALL persist a structured failure when the condition blocks active provisioning progress
- **AND** the persisted failure SHALL preserve stable UI-safe reason and recovery action metadata

#### Scenario: Worker readiness lag is non-terminal

- **WHEN** the Provisioner Worker is temporarily unreachable while Native can safely continue observing the active provisioning pod
- **THEN** Workspace Provisioning SHALL continue reporting running or readiness progress
- **AND** Workspace Provisioning MUST NOT persist failure state for normal worker startup lag

#### Scenario: Worker terminal or contract failure is persisted

- **WHEN** the Provisioner Worker is unauthorized, returns an invalid unrecoverable response, reports terminal failure, or otherwise violates the worker API contract during environment preparation
- **THEN** Workspace Provisioning SHALL persist a structured failure with provisioner-worker source and inspect-oriented or recovery-oriented action
- **AND** persisted failure metadata MUST remain stable, UI-safe, and secret-safe
- **AND** when the persisted failure is available, Workspace Provisioning SHALL return authoritative Workspace metadata and progress instead of a direct command error

#### Scenario: Granular worker preparation subtype is not a command error

- **WHEN** a terminal Provisioner Worker preparation subtype occurs during active provisioning sync
- **THEN** Workspace Provisioning SHALL persist the granular worker failure code on the Workspace
- **AND** Workspace Provisioning SHALL derive failed progress from the persisted Workspace failure
- **AND** Workspace Provisioning MUST NOT allow that subtype to escape as `NativeCommandErrorCode::ProvisionerWorkerFailed` during normal sync handling

#### Scenario: Worker token is missing or invalid during preparation

- **WHEN** Workspace Provisioning needs a stored Provisioner Worker bearer token to communicate with an active provisioning pod
- **AND** the stored token is missing or invalid
- **THEN** Workspace Provisioning SHALL persist a structured failure as native state inconsistency
- **AND** command responses and persisted failure metadata MUST NOT include the token value

### Requirement: Provisioning maps resource errors explicitly

Workspace Provisioning SHALL explicitly map `WorkspaceResourceError` categories into immediate command errors, non-mutating progress, or persisted `WorkspaceProvisioningFailure` records.

#### Scenario: Resource error escapes as command error

- **WHEN** Workspace Resources returns a catalog, secret/keyring, transient provider availability, conflict, or other category that does not require durable Workspace recovery state
- **THEN** Workspace Provisioning SHALL map it to the corresponding `WorkspaceProvisioningError`
- **AND** the command boundary SHALL map it into stable UI-safe command metadata

#### Scenario: Resource error becomes persisted failure

- **WHEN** Workspace Resources returns provider operation uncertainty, provider resource missing, orphaned resource, cleanup failure, terminal worker failure, or token lifecycle state inconsistency
- **THEN** Workspace Provisioning SHALL persist the corresponding structured Workspace failure when catalog persistence is available
- **AND** Workspace progress SHALL expose the persisted failure through generated binding-safe types

#### Scenario: Mapping behavior is covered by tests

- **WHEN** regression tests exercise representative `WorkspaceResourceError` categories
- **THEN** each category SHALL assert whether it returns as a command error or persists as Workspace failure

### Requirement: Expose endpoint setup as a single provisioning phase
Workspace Provisioning SHALL expose endpoint setup through the `creating_endpoint` phase without exposing RunPod endpoint template creation, observation, discovery, or validation as a separate domain, command, frontend, or persisted failure phase.

#### Scenario: Progress reaches endpoint setup before template exists
- **WHEN** a provisioning Workspace has a prepared environment, no active Provisioning Pod, no RunPod endpoint template snapshot, and no Serverless Endpoint snapshot
- **THEN** Workspace Provisioning Progress SHALL report status `running`
- **AND** Workspace Provisioning Progress SHALL report phase `creating_endpoint`
- **AND** Workspace Provisioning Progress MUST NOT report phase `creating_endpoint_template`

#### Scenario: Template failure is reported as endpoint creation failure
- **WHEN** RunPod endpoint template creation, observation, discovery, or validation fails in a way that requires persisted Workspace failure metadata
- **THEN** the Native Layer SHALL persist structured failure detail with phase `creating_endpoint`
- **AND** the Native Layer MUST NOT persist phase `creating_endpoint_template`
- **AND** the failure code, source, retryability, recovery action, and known cleanup metadata SHALL preserve the existing recovery semantics for the failed template operation

#### Scenario: Legacy template phase is read
- **WHEN** Workspace Provisioning reads existing persisted failure metadata whose phase value is `creating_endpoint_template`
- **THEN** the Native Layer SHALL treat the value as `creating_endpoint`
- **AND** subsequent command responses and generated frontend bindings MUST NOT expose `creating_endpoint_template`
- **AND** subsequent writes MUST NOT emit `creating_endpoint_template`

### Requirement: Report normalized Workspace Provisioning progress
Workspace Provisioning SHALL report `WorkspaceProvisioningProgress.percent` as total provisioning progress toward a Ready Workspace whenever a percentage is present.

#### Scenario: Idle and terminal progress use fixed percentages
- **WHEN** Workspace Provisioning Progress is derived for a Draft Workspace
- **THEN** the progress status SHALL be `idle`
- **AND** the progress phase SHALL be `not_started`
- **AND** the progress percent SHALL be `0`
- **WHEN** Workspace Provisioning Progress is derived for a Ready Workspace
- **THEN** the progress status SHALL be `completed`
- **AND** the progress phase SHALL be `completed`
- **AND** the progress percent SHALL be `100`

#### Scenario: Provider orchestration phases use total-progress anchors
- **WHEN** Workspace Provisioning Progress is derived for active provider orchestration
- **THEN** phase `creating_volume` SHALL report percent `0`
- **AND** phase `starting_provisioning_pod` SHALL report percent `10`
- **AND** phase `creating_endpoint` SHALL report percent `90`
- **AND** phase `validating_readiness` SHALL report percent `98`

#### Scenario: Worker preparation progress is scaled into total progress
- **WHEN** the Provisioner Worker reports running preparation progress for the active Workspace job
- **AND** the worker progress percent is `0`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `40`
- **WHEN** the worker progress percent is `50`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `65`
- **WHEN** the worker progress percent is `100`
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `90`

#### Scenario: Missing worker percent uses preparation lower bound
- **WHEN** the Provisioner Worker reports a running preparation status without a progress percentage
- **THEN** Workspace Provisioning Progress SHALL report phase `preparing_environment`
- **AND** the progress percent SHALL be `40`

#### Scenario: Cleanup and failure are outside the ready-progress scale
- **WHEN** Workspace Provisioning Progress is derived for cancellation cleanup
- **THEN** the progress phase SHALL be `cleaning_up`
- **AND** the progress percent SHALL be absent
- **WHEN** Workspace Provisioning Progress is derived for a Failed Workspace
- **THEN** the progress status SHALL be `failed`
- **AND** the progress phase SHALL be `failed`
- **AND** the progress percent SHALL be absent
