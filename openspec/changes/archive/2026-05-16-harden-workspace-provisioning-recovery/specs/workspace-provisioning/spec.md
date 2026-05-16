## MODIFIED Requirements

### Requirement: Provision RunPod Network Volume

Workspace Provisioning SHALL create, discover, adopt, or observe one RunPod network volume for the Workspace and persist its snapshot before later compute resources depend on it, without blindly creating duplicate provider volumes when create results are indeterminate.

#### Scenario: Network volume is created

- **WHEN** a provisioning Workspace has no Persistent Storage Volume snapshot
- **AND** provider discovery finds no safe Workspace-correlated RunPod network volume
- **THEN** the Native Layer SHALL create a RunPod network volume in the selected data center with the requested size
- **AND** the Native Layer SHALL persist the provider resource id, data center id, provisioned size, provider status, and mount path `/workspace`
- **AND** the Native Layer SHALL use the Workspace identifier as the stable correlation value when provider naming supports it

#### Scenario: Existing volume snapshot is refreshed

- **WHEN** a provisioning Workspace already has a Persistent Storage Volume snapshot
- **THEN** the Native Layer SHALL observe the corresponding RunPod network volume before using it for later readiness decisions
- **AND** the Native Layer SHALL update the snapshot status from the provider observation
- **AND** the Native Layer MUST NOT blindly create a second network volume

#### Scenario: Existing correlated volume is adopted before create

- **WHEN** a provisioning Workspace has no Persistent Storage Volume snapshot
- **AND** provider discovery finds exactly one RunPod network volume correlated to the Workspace by stable Workspace-derived volume name and expected placement metadata
- **THEN** the Native Layer SHALL persist that volume as the Persistent Storage Volume snapshot
- **AND** the Native Layer MUST NOT create another RunPod network volume
- **AND** the persisted snapshot SHALL include the provider resource id, data center id, provisioned size, provider status, and mount path `/workspace`

#### Scenario: Volume creation result is indeterminate

- **WHEN** a RunPod network volume creation request times out or returns an indeterminate response
- **THEN** the Native Layer SHALL inspect durable Workspace metadata and discoverable provider resources before retrying creation
- **AND** the Native Layer SHALL adopt exactly one safe Workspace-correlated volume when provider discovery proves that one exists
- **AND** the Native Layer SHALL mark the Workspace `failed` when it cannot identify exactly one safe Workspace-correlated volume
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

#### Scenario: Provisioning pod create result is indeterminate

- **WHEN** a RunPod provisioning pod creation request times out or returns an indeterminate response after the per-workspace Provisioner Worker bearer token is stored
- **THEN** the Native Layer SHALL discover live RunPod pods correlated to the Workspace by stable Workspace-derived pod name and network volume id before retrying creation
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

### Requirement: Provision RunPod Serverless Template

Workspace Provisioning SHALL create, discover, adopt, or observe one per-user RunPod serverless template for the Endpoint Worker and persist its provider-specific template id before creating the Serverless Endpoint, without blindly creating duplicate templates when create results are indeterminate.

#### Scenario: Serverless template is created

- **WHEN** a provisioning Workspace has a prepared environment, no active provisioning pod, no RunPod endpoint template snapshot, and no safe Workspace-correlated template exists
- **THEN** the Native Layer SHALL create a RunPod serverless template in the user's RunPod account using the configured RunPod Endpoint Worker image
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
- **THEN** the Native Layer SHALL create a RunPod Serverless Endpoint using the persisted `template_id`, selected GPU, selected data center, network volume id, and configured Endpoint Worker runtime values
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
