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

Workspace Provisioning SHALL create or observe one RunPod network volume for the Workspace and persist its snapshot before later compute resources depend on it.

#### Scenario: Network volume is created

- **WHEN** a provisioning Workspace has no Persistent Storage Volume snapshot
- **THEN** the Native Layer SHALL create a RunPod network volume in the selected data center with the requested size
- **AND** the Native Layer SHALL persist the provider resource id, data center id, provisioned size, provider status, and mount path `/workspace`
- **AND** the Native Layer SHALL use the Workspace identifier as the stable correlation value when provider naming supports it

#### Scenario: Existing volume snapshot is refreshed

- **WHEN** a provisioning Workspace already has a Persistent Storage Volume snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod network volume before using it for later readiness decisions
- **AND** the Native Layer SHALL update the snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second network volume

#### Scenario: Volume creation result is indeterminate

- **WHEN** a RunPod network volume creation request times out or returns an indeterminate response
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discoverable provider resources before retrying creation
- **AND** the Native Layer SHALL mark the Workspace `failed` when it cannot identify exactly one safe Workspace-correlated volume
- **AND** known cleanup metadata SHALL be retained

### Requirement: Provision Temporary RunPod Provisioning Pod

Workspace Provisioning SHALL create, observe, and terminate a temporary RunPod provisioning pod that mounts the Workspace network volume at `/workspace` and runs the Provisioner Worker image.

#### Scenario: Provisioning pod is created

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot and no active Provisioning Pod snapshot
- **THEN** the Native Layer SHALL generate and store a per-workspace Provisioner Worker bearer token in secure storage
- **AND** the Native Layer SHALL create a RunPod pod using the configured Provisioner Worker image, selected GPU, selected data center, network volume id, and mount path `/workspace`
- **AND** the Native Layer SHALL inject the bearer token into the pod environment only for the Provisioner Worker runtime
- **AND** the Native Layer SHALL persist the active Provisioning Pod snapshot after the provider resource id is known

#### Scenario: Provisioning pod is observed

- **WHEN** a provisioning Workspace has an active Provisioning Pod snapshot
- **THEN** the Native Layer SHALL observe the RunPod pod status before contacting the Provisioner Worker
- **AND** the Native Layer SHALL update the active Provisioning Pod snapshot from the provider observation
- **AND** the Native Layer SHALL mark the Workspace `failed` if the pod is failed, terminated unexpectedly, or unreachable in a way that prevents safe continuation

#### Scenario: Provisioning pod is terminated after preparation

- **WHEN** the Provisioner Worker has reported terminal success and the prepared environment timestamp is durable
- **THEN** the Native Layer SHALL delete or terminate the RunPod provisioning pod
- **AND** the Native Layer SHALL move the terminal pod snapshot to the last Provisioning Pod snapshot
- **AND** the Native Layer SHALL clear the active Provisioning Pod snapshot after termination is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token after the pod is confirmed no longer needed

### Requirement: Drive Provisioner Worker Preparation

Workspace Provisioning SHALL start and observe the Provisioner Worker job using the selected Workflow Preset and a per-workspace bearer token.

#### Scenario: Provisioner Worker job starts

- **WHEN** the active Provisioning Pod is running and the Provisioner Worker is reachable and idle
- **THEN** the Native Layer SHALL call `POST /start` with the Workspace identifier and selected Workflow Preset
- **AND** the request SHALL include `Authorization: Bearer <stored-token>`
- **AND** the Native Layer MUST NOT include Provider API Keys in the worker request

#### Scenario: Provisioner Worker progress is reported

- **WHEN** the Provisioner Worker reports `running` or `cancelling` status for the active Workspace job
- **THEN** the Native Layer SHALL derive Workspace Provisioning Progress from the worker status, phase, progress percentage, and UI-safe diagnostic metadata
- **AND** the Native Layer MUST NOT persist worker progress as authoritative lifecycle state

#### Scenario: Provisioner Worker succeeds

- **WHEN** the Provisioner Worker reports terminal success for the active Workspace job
- **THEN** the Native Layer SHALL persist the environment prepared timestamp
- **AND** later readiness validation MAY depend on the prepared environment metadata

#### Scenario: Provisioner Worker fails

- **WHEN** the Provisioner Worker reports terminal failure or returns an unrecoverable worker API error
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume and provisioning pod snapshots for future cleanup
- **AND** returned diagnostics SHALL be UI-safe and MUST NOT contain bearer tokens, Provider API Keys, raw command output, stack traces, or environment dumps

### Requirement: Provision RunPod Serverless Template

Workspace Provisioning SHALL create or observe one per-user RunPod serverless template for the Endpoint Worker and persist its provider-specific template id before creating the Serverless Endpoint.

#### Scenario: Serverless template is created

- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, and no RunPod endpoint template snapshot
- **THEN** the Native Layer SHALL create a RunPod serverless template in the user's RunPod account using the configured RunPod Endpoint Worker image
- **AND** the template SHALL use mount path `/workspace`
- **AND** the Native Layer SHALL persist the RunPod `template_id`, image reference, mount path, and provider status before creating an endpoint from that template

#### Scenario: Template snapshot is observed

- **WHEN** a provisioning Workspace already has a RunPod endpoint template snapshot
- **THEN** the Native Layer SHALL observe or validate that template before creating or validating the Serverless Endpoint
- **AND** the Native Layer MUST NOT blindly create a second template

#### Scenario: Template creation fails after provider success

- **WHEN** RunPod creates a serverless template but a later provider action fails
- **THEN** the Native Layer SHALL retain the persisted template snapshot
- **AND** future cleanup SHALL have enough metadata to delete the template even though Workspace Resource Cleanup is out of scope for this change

### Requirement: Provision RunPod Serverless Endpoint

Workspace Provisioning SHALL create or observe one RunPod Serverless Endpoint from the persisted per-user template and attach the Workspace network volume.

#### Scenario: Serverless endpoint is created

- **WHEN** a provisioning Workspace has a ready Persistent Storage Volume snapshot, a persisted RunPod endpoint template snapshot, and no Serverless Endpoint snapshot
- **THEN** the Native Layer SHALL create a RunPod Serverless Endpoint using the persisted `template_id`, selected GPU, selected data center, network volume id, and configured Endpoint Worker runtime values
- **AND** the endpoint SHALL use `workersMin = 0`, `workersMax = 1`, `scalerType = "REQUEST_COUNT"`, and `scalerValue = 1`
- **AND** the endpoint `idleTimeout` SHALL be set from the persisted RunPod Placement Plan endpoint keep-alive seconds
- **AND** the Native Layer SHALL persist the endpoint provider resource id, data center id, selected GPU id, provider status, and endpoint invoke URL after the provider resource id is known

#### Scenario: Existing endpoint snapshot is refreshed

- **WHEN** a provisioning Workspace already has a Serverless Endpoint snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod Serverless Endpoint before readiness validation
- **AND** the Native Layer SHALL update the endpoint snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second endpoint

#### Scenario: Endpoint setup fails

- **WHEN** endpoint creation, endpoint observation, or endpoint metadata validation fails in a way that prevents safe continuation
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain known volume, template, and endpoint metadata for future cleanup

### Requirement: Validate Provisioning Readiness

Workspace Provisioning SHALL mark a Workspace `ready` only after required provider resources and the prepared runtime environment are durably represented and provider observations confirm readiness.

#### Scenario: Workspace becomes ready

- **WHEN** the Persistent Storage Volume, RunPod endpoint template, and Serverless Endpoint snapshots are persisted and provider observations confirm the required resources still exist in acceptable states
- **AND** the prepared environment timestamp is durable
- **AND** no active Provisioning Pod snapshot remains
- **THEN** the Native Layer SHALL persist the Workspace lifecycle state as `ready`
- **AND** Workspace Provisioning Progress SHALL report status `completed`

#### Scenario: Unknown resource status blocks readiness

- **WHEN** any required provider resource has status `unknown`, is missing, or cannot be confirmed by provider observation
- **THEN** the Native Layer MUST NOT mark the Workspace `ready`
- **AND** the Native Layer SHALL mark the Workspace `failed` when safe continuation or later validation is impossible

#### Scenario: Readiness excludes generation

- **WHEN** Workspace Provisioning validates the persistent runtime entry point
- **THEN** the Native Layer SHALL validate provider metadata and no-job endpoint health or status information only
- **AND** the Native Layer MUST NOT submit a generation request to the Endpoint Worker

### Requirement: Cancel Workspace Provisioning

Workspace Provisioning SHALL support user cancellation while a Workspace is in `provisioning` by using shared known-resource cleanup behavior and returning the Workspace to `draft` only after cancellation cleanup succeeds.

#### Scenario: Cancellation succeeds

- **WHEN** the Client cancels provisioning for a Workspace in `provisioning`
- **THEN** the Native Layer SHALL invoke shared cleanup behavior for the Workspace-owned Provider Resources known from authoritative Workspace metadata
- **AND** shared cleanup SHALL cancel the active Provisioner Worker job when a matching active pod and token exist
- **AND** shared cleanup SHALL delete the Serverless Endpoint, RunPod endpoint template, Provisioning Pod, and Persistent Storage Volume resources known from Workspace metadata when they exist
- **AND** the Native Layer SHALL tolerate already-missing provider resources
- **AND** the Native Layer SHALL clear provisioning snapshots and return the Workspace lifecycle state to `draft` only after cleanup is confirmed
- **AND** the Native Layer SHALL delete the stored Provisioner Worker bearer token when no active provisioning pod remains

#### Scenario: Cancellation cleanup is incomplete

- **WHEN** cancellation cannot confirm deletion of all known Provider Resources
- **THEN** the Native Layer SHALL mark the Workspace `failed`
- **AND** the Native Layer SHALL retain all known Provider Resource and RunPod template metadata for future Workspace Resource Cleanup

#### Scenario: Shared cleanup preserves policy-specific final mutation

- **WHEN** shared known-resource cleanup succeeds for a cancellation request
- **THEN** Workspace Provisioning SHALL apply the cancellation policy by clearing provisioning snapshots and returning the existing Workspace Catalog entry to `draft`
- **AND** shared cleanup behavior MUST NOT delete the Workspace Catalog entry during provisioning cancellation

### Requirement: Reuse Known Workspace Resource Cleanup

The Native Layer SHALL centralize deletion of known Workspace-owned provisioning resources so Workspace Provisioning cancellation and future Workspace Resource Cleanup use the same provider deletion semantics.

#### Scenario: Known resources are cleaned in dependency-safe order

- **WHEN** shared cleanup receives Workspace metadata with known provisioning resources
- **THEN** it SHALL attempt cleanup in dependency-safe order: active Provisioner Worker job, Serverless Endpoint, RunPod endpoint template, active Provisioning Pod, Persistent Storage Volume, and per-workspace Provisioner Worker bearer token
- **AND** it SHALL tolerate already-missing Provider Resources
- **AND** it SHALL report cleanup success only after all known resources are deleted or confirmed missing

#### Scenario: Cleanup final state is chosen by caller policy

- **WHEN** shared cleanup returns a result to a caller
- **THEN** the caller SHALL decide the final local Workspace Catalog mutation
- **AND** Workspace Provisioning cancellation SHALL return the Workspace to `draft` on cleanup success
- **AND** future Workspace Resource Cleanup MAY delete the Workspace Catalog entry on cleanup success
- **AND** cleanup failure SHALL preserve known metadata for later recovery

### Requirement: Preserve Secret Safety During Provisioning

Workspace Provisioning SHALL keep Provider API Keys and Provisioner Worker bearer tokens out of Workspace metadata, command responses, logs, diagnostics, and generated frontend bindings.

#### Scenario: Provider API Key is used

- **WHEN** Workspace Provisioning calls RunPod
- **THEN** the Native Layer SHALL read the Provider API Key from secure storage through the provider registry
- **AND** the Provider API Key MUST NOT be written to Workspace Catalog metadata, command responses, logs, diagnostics, or generated frontend types

#### Scenario: Provisioner bearer token is stored

- **WHEN** Workspace Provisioning creates a temporary Provisioning Pod
- **THEN** the Native Layer SHALL store the Provisioner Worker bearer token in a per-workspace keyring entry separate from Provider API Key storage
- **AND** the bearer token MUST NOT be written to Workspace Catalog metadata, command responses, logs, diagnostics, or generated frontend types

#### Scenario: Provisioner bearer token is removed

- **WHEN** the active Provisioning Pod is confirmed terminated, successful cancellation returns the Workspace to `draft`, or future cleanup removes the Workspace
- **THEN** the Native Layer SHALL delete the per-workspace Provisioner Worker bearer token when it exists
- **AND** missing token entries SHALL be tolerated after no active provisioning pod remains

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
- **AND** command responses, Workspace metadata, logs, diagnostics, and generated frontend bindings MUST NOT expose those secrets

