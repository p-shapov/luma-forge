## Context

LumaForge already has native-owned GPU Cloud Provider Setup and Workspace Setup. Workspace Setup persists a complete `Draft` Workspace with a selected RunPod placement plan and Workflow Preset, but no native service currently provisions the remote infrastructure needed to run that workspace.

The worker side is already specified and implemented separately: the Provisioner Worker prepares a mounted ComfyUI volume through an authenticated HTTP API, and the RunPod Endpoint Worker serves generation from a prepared volume. This change connects those pieces from the Tauri native layer without adding a hosted backend or moving orchestration into React.

Current native boundaries shape the design:

- Domain models remain independent from Tauri command DTOs and provider transport DTOs.
- Provider clients return provider-local errors.
- `ProviderClientRegistry` adapts provider-local errors into use-case errors.
- Secrets are stored through `src-tauri/src/secrets/`, not in Workspace metadata.
- Workspace Catalog currently supports reads and inserts only, so provisioning requires durable update operations.
- Workspace Setup currently exposes provider inventory as the placement read surface; keep-alive configuration means that surface must become provider placement options rather than raw inventory only.

## Goals / Non-Goals

**Goals:**

- Provision one saved `Draft` Workspace into `Ready` through Native-owned orchestration.
- Keep provisioning idempotent and resumable by deriving each action from durable Workspace metadata.
- Persist enough RunPod resource metadata to support future cleanup, including per-user serverless template identifiers.
- Persist the RunPod endpoint keep-alive selection as provider-specific placement data and use it during endpoint creation.
- Use the existing provider registry and RunPod client boundary instead of creating a second provider hierarchy.
- Store per-workspace Provisioner Worker bearer tokens in keyring and delete them after the provisioning pod is confirmed no longer needed.
- Introduce reusable cleanup behavior for known Workspace resources so provisioning cancellation and future Workspace Resource Cleanup share deletion semantics.
- Return UI-safe Workspace Provisioning Progress derived from authoritative Workspace metadata and provider/worker observations.

**Non-Goals:**

- React UI implementation.
- Generation commands against the ready endpoint.
- Full Workspace Resource Cleanup for failed or ready workspaces.
- Repairing `Failed` Workspaces.
- Multi-provider provisioning beyond RunPod.
- App-owner RunPod serverless templates.
- Post-provisioning endpoint keep-alive update commands.

## Decisions

### Use a sync-driven durable state machine

Workspace Provisioning will expose initiate, sync, and cancel operations. Initiate validates a `Draft` Workspace, transitions it to `Provisioning`, and returns progress. Sync performs at most one safe action derived from durable Workspace state, then returns the updated Workspace and derived progress.

Rationale: provider mutations can time out or the app can exit mid-flow. A durable state machine avoids hidden in-memory progress and lets the next sync resume from persisted checkpoints.

Alternative considered: run the whole provisioning flow as one long command. This is simpler at the call boundary but makes app exit, duplicate command calls, partial provider success, and cancellation harder to handle safely.

### Keep the Workspace aggregate authoritative

Workspace metadata remains the aggregate root for provisioning checkpoints. Provider Resource snapshots, lifecycle state, environment-prepared timestamp, and provider-specific provisioning metadata are persisted as part of the Workspace record.

Rationale: this matches current Workspace Catalog behavior, where serialized Workspace JSON is authoritative and indexed columns are consistency checks. It also keeps readiness decisions local to one durable aggregate.

Alternative considered: create separate SQLite tables for provider resources immediately. That may become useful later, but for v1 it adds joins and migration complexity without changing the aggregate consistency boundary.

### Add provider-specific RunPod provisioning snapshot metadata

RunPod serverless endpoint deployment requires a serverless template. Because app-owner templates are not assumed usable across user accounts, provisioning creates a per-user RunPod template. The created `template_id` must be persisted before endpoint creation.

Recommended domain shape:

```text
Workspace
  provider_provisioning_snapshot: Option<ProviderProvisioningSnapshot>

ProviderProvisioningSnapshot::Runpod
  endpoint_template_snapshot: Option<RunPodEndpointTemplateSnapshot>
```

The template snapshot should include at least `template_id`, status, endpoint worker image ref, and `mount_path = "/workspace"`.

Rationale: if template creation succeeds but endpoint creation fails, future cleanup still needs the template id. Storing the template id only inside the endpoint snapshot would lose cleanup metadata for that partial failure.

Alternative considered: add `template_id` directly to `ServerlessEndpointSnapshot`. That couples endpoint metadata to a provider-specific prerequisite and cannot represent "template exists, endpoint does not."

### Extend existing provider registry and RunPod client

The Workspace Provisioning service will define a `ProviderProvisioningGateway` trait. `ProviderClientRegistry` will implement it and dispatch RunPod operations to the existing `RunPodClient`.

`RunPodClient` will add REST methods for:

- network volume create/get/delete
- pod create/get/delete
- template create/get/delete
- endpoint create/get/delete

Rationale: this preserves the current native boundary pattern. Provider clients stay provider-local, and the registry maps provider errors into use-case errors.

Alternative considered: create `provider/provisioning` and `provider/runpod/provisioning` module hierarchies. That is clean for a larger multi-provider system, but it is premature while RunPod is the only provider and the existing registry is already the provider facade.

### Use `/workspace` as the canonical mount path

Both the temporary provisioning pod and the serverless endpoint template will mount the network volume at `/workspace`. Endpoint worker runtime configuration should use the same mount path.

Rationale: the Provisioner Worker writes runtime metadata and the Endpoint Worker validates it. A single canonical path avoids path translation in manifests, worker config, and readiness validation.

Alternative considered: use RunPod's default serverless mount path and configure only the endpoint worker to read there. That would diverge from the provisioner pod and increase the chance of manifest/path mismatch.

### Rename provider inventory command to provider placement options

Workspace Setup will replace `get_provider_inventory` with `get_provider_placement_options`. The new command response includes live provider inventory plus provider placement capability metadata:

```text
GetProviderPlacementOptionsResponse
  provider_inventory: ProviderInventory
  placement_capabilities: ProviderPlacementCapabilities

ProviderPlacementCapabilities
  endpoint_keep_alive: EndpointKeepAliveCapability

EndpointKeepAliveCapability
  supported: true
  default_seconds
  min_seconds
  max_seconds

EndpointKeepAliveCapability
  supported: false
```

For RunPod v1, endpoint keep-alive is supported with `default_seconds = 5`, `min_seconds = 5`, and `max_seconds = 3600`.

Rationale: data centers and GPUs are inventory, but keep-alive range/defaults are provider configuration capabilities. Keeping them in one placement-options command gives the client one read surface for Workspace Setup without hardcoding provider-specific rules in React.

Alternative considered: keep the command name `get_provider_inventory` and append capabilities to its response. That avoids a binding rename but makes the command name misleading as placement concerns grow.

### Store endpoint keep-alive in provider-specific Placement Plan data

RunPod `PlacementPlan` data will include the selected endpoint keep-alive value:

```text
PlacementPlan::Runpod
  selected_datacenter_id
  selected_gpu_id
  persistent_storage_volume_size_bytes
  endpoint_keep_alive_seconds
  selected_workflow_preset
```

The Native Layer validates RunPod keep-alive as provider-specific placement data using RunPod's allowed range `5..=3600`. Workspace Setup defaults the value from placement capabilities, but Workspace creation still validates the submitted value authoritatively.

Future providers that do not support endpoint keep-alive should expose `endpoint_keep_alive.supported = false` in placement capabilities and omit any keep-alive field from their provider-specific Placement Plan variant.

Rationale: keep-alive is not a provider-neutral concept. Persisting it inside the RunPod placement variant keeps the durable plan explicit, lets provisioning create endpoints from Workspace metadata, and leaves room for providers where this setting is absent or named differently.

Alternative considered: store a provider-neutral optional keep-alive field on every Placement Plan. That would force unsupported providers to persist `null` or `unsupported` state and blur provider-specific validation rules.

### Use conservative RunPod endpoint autoscaling defaults

V1 RunPod Serverless Endpoints will use queue-based endpoint workers with these defaults:

```text
workersMin = 0
workersMax = 1
idleTimeout = PlacementPlan::Runpod.endpoint_keep_alive_seconds
scalerType = "REQUEST_COUNT"
scalerValue = 1
```

Rationale: LumaForge v1 is a single-user desktop workflow and generated image requests are relatively expensive, stateful, and tied to one prepared Workspace volume. `workersMin = 0` avoids charging users for an always-on GPU worker. `workersMax = 1` avoids concurrent endpoint workers writing against the same prepared volume and keeps cost bounded. `REQUEST_COUNT` with `scalerValue = 1` asks RunPod to scale to one worker as soon as queued work exists, which avoids the deliberate queue-delay threshold on the first request. The default keep-alive value is the RunPod minimum `5` seconds to minimize paid warm-idle time by default.

Alternative considered: fixed `idleTimeout = 300`, which reduces repeated cold starts during interactive image iteration but causes users to pay for up to five minutes of warm idle time after one generation. Another alternative is `workersMin = 1`, which improves first-request latency but creates ongoing idle GPU charges and does not fit a cost-conscious local desktop v1.

### Defer post-provisioning keep-alive edits

This change will persist keep-alive selection and use it during endpoint creation, but it will not add a command to edit keep-alive after a Workspace becomes `Ready`.

A later change can add an update command that calls the provider first and persists the new local setting only after provider success.

Rationale: persisting the selected value now is the necessary foundation. Updating ready endpoint configuration introduces a separate lifecycle and rollback problem that should be designed independently.

### Store per-workspace provisioner tokens in the existing secrets module

Provisioning will extend `src-tauri/src/secrets/` with per-workspace Provisioner Worker bearer token operations. Tokens must use a separate keyring scope/account from Provider API Keys.

Tokens are retained while an active provisioning pod may need observation or cancellation. They are deleted after the provisioning pod is confirmed terminated and the Workspace reaches `Ready`, after successful cancellation returns the Workspace to `Draft`, or during future workspace cleanup.

Rationale: storing tokens only in memory breaks crash recovery, while storing them in Workspace metadata would expose secrets through persistence, diagnostics, and generated command outputs.

Alternative considered: create a separate top-level `provisioning_secrets` module. The existing `secrets` module is already the infrastructure boundary, so extending it keeps secret storage centralized.

### Make command progress derived, not authoritative

Command responses will return authoritative Workspace metadata and a derived `WorkspaceProvisioningProgress`. Progress status and phase are computed from lifecycle, snapshots, provider observations, and worker status.

Rationale: React may use progress to render and decide whether to sync again, but it must not decide provisioning actions. Durable Workspace state is the source of truth.

Alternative considered: persist a separate progress record. That duplicates state and risks conflicts between progress and Workspace snapshots.

### Factor known-resource cleanup into a neutral module

Provisioning cancellation and future Workspace Resource Cleanup both need to delete the same known Workspace-owned provider resources: Serverless Endpoint, RunPod serverless template, active Provisioning Pod, Persistent Storage Volume, and the per-workspace Provisioner Worker bearer token. This change will introduce shared cleanup behavior that operates on authoritative Workspace metadata and returns a cleanup result without deciding the final local Workspace mutation.

The shared cleanup behavior will live in `src-tauri/src/workspace_resource_cleanup` even before a public Workspace Resource Cleanup command exists.

The cleanup component should delete resources in dependency-safe order:

```text
1. Cancel active Provisioner Worker job when an active pod and token exist
2. Delete Serverless Endpoint
3. Delete RunPod serverless template
4. Delete active Provisioning Pod
5. Delete Persistent Storage Volume
6. Delete per-workspace Provisioner Worker bearer token once no active pod remains
```

Provisioning cancellation uses that cleanup result with a return-to-draft policy:

```text
cleanup success -> clear provisioning snapshots, lifecycle = Draft
cleanup failure -> lifecycle = Failed, retain known metadata
```

Future Workspace Resource Cleanup should use the same cleanup behavior with a delete-workspace policy:

```text
cleanup success -> delete Workspace Catalog entry
cleanup failure -> keep Workspace row, lifecycle = Failed, retain known metadata
```

Rationale: duplicated deletion logic would drift and make cancellation safer or less safe than cleanup. A neutral module keeps idempotent provider deletion, already-missing resource tolerance, template deletion, and token deletion semantics consistent, and avoids baking cleanup ownership into `workspace_provisioning`.

Alternative considered: implement cancellation deletion inline inside `workspace_provisioning` and defer shared cleanup until the public cleanup command exists. That is faster initially but risks building two subtly different cleanup paths around the same provider resources. Another alternative is a private cleanup submodule under `workspace_provisioning`, but that makes the future cleanup command depend on a provisioning-owned module.

## Risks / Trade-offs

- RunPod create calls may succeed while the response is lost -> Persist snapshots immediately after resource ids are known, use stable workspace-derived names/correlation values, and fail closed if exactly one safe provider match cannot be identified.
- Per-user serverless template creation may leak templates on partial failure -> Persist `template_id` before endpoint creation and keep cleanup metadata even though full cleanup is out of scope for this change.
- Worker bearer token may outlive its pod -> Delete it when pod termination is confirmed, and never expose it in Workspace metadata, command responses, or logs.
- Workspace Catalog JSON schema changes may break existing development data -> Add a persistence migration/version update and fail closed on corrupt or future-version data.
- Command rename may break generated frontend callers -> Update generated bindings and command tests in the same change; React UI implementation remains out of scope.
- Provider status mapping may mark unknown resources as ready too aggressively -> Require explicit ready/running states per resource type before readiness decisions.
- One sync per Workspace may still race across processes if multiple app instances run -> Use in-process coordinator for v1 and rely on SQLite transaction guards for durable state consistency.
- RunPod REST and GraphQL error shapes differ -> Keep REST DTOs and mapping inside `provider/runpod`, return provider-local error categories to the registry.
- Cancellation cleanup may duplicate future Workspace Resource Cleanup -> Introduce shared known-resource cleanup now and let provisioning cancellation provide only the final return-to-draft policy.

## Migration Plan

1. Rename Workspace Setup placement read command and add provider placement capability data.
2. Add RunPod endpoint keep-alive to provider-specific Placement Plan data and validation.
3. Add Workspace domain fields and Workspace Catalog migration for provider-specific provisioning metadata.
4. Add transactional Workspace Catalog update operations before provider mutations are introduced.
5. Add provider registry and RunPod REST provisioning calls behind tests.
6. Add Provisioner Worker client and secret-store token operations.
7. Add shared known-resource cleanup behavior before wiring provisioning cancellation.
8. Add Workspace Provisioning service and commands in small state-machine increments.
9. Keep failure behavior cleanup-first: preserve known snapshots and mark Workspace `Failed` when safe continuation is impossible.

Rollback during development is local-data sensitive because Workspace JSON shape changes. If a migration lands and must be reverted before release, remove development catalog data or add a follow-up migration that tolerates the interim shape.

## Open Questions

- Exact RunPod endpoint scaler defaults and shared cleanup module ownership are decided above. No blocking open questions remain for implementation planning.
