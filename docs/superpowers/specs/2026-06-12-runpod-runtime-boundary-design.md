# RunPod Runtime Boundary Design

## Summary

Remove `GpuCloudProviderId` and the provider-shaped `provisioned_remote` abstraction. The current implementation is not a generic GPU cloud provider layer; it is a RunPod-specific workflow that creates a network volume, starts a provisioner pod, creates a Serverless template and endpoint, then cleans those RunPod resources up.

The target model should say that directly. Workspaces keep a top-level `WorkspaceRuntime` enum for future runtime families, but the current remote runtime variant becomes `Runpod`. Runtime-specific placement, requirements, resources, commands, and persistence use RunPod vocabulary and RunPod units.

## Current State

The current domain model has several generic names that do not match the implementation:

- `GpuCloudProviderId` has only `Runpod`.
- `RemoteRuntimeRequirements` contains `provider_requirements`.
- `ProvisionedRemoteRuntime` derives provider identity from `RemotePlacementPlan`.
- `RemotePlacementPlan` persists `gpu_cloud_provider_id`, `gpu_id`, and `volume_size_bytes`.
- `ProvisionedRemoteProviderRegistry` dispatches to one registered RunPod provider.
- The command `create_workspace` accepts a `remote_placement`, but that placement is valid only for the RunPod lifecycle.
- SQLite persists a `provider_id` column even though there is no provider choice.

The lower RunPod adapter already uses concrete RunPod API concepts: network volumes, pods, Serverless templates, endpoints, `dataCenterIds`, `gpuTypeIds`, `networkVolumeId`, `templateId`, and volume size in GB.

## Goals

- Delete `GpuCloudProviderId` as a domain and command concept.
- Rename the active runtime from `ProvisionedRemoteRuntime` to `RunpodRuntime`.
- Replace generic provider registry and provider traits with a concrete RunPod runtime client boundary.
- Rename command contracts so RunPod-specific commands are explicit.
- Store RunPod placement and resources in RunPod units and field names.
- Update workflow revision requirements to describe RunPod runtime requirements directly.
- Persist `template_id` as part of RunPod resources.
- Keep `WorkspaceRuntime` as the product-level runtime enum for future local or hosted runtimes.

## Non-Goals

- Add compatibility paths for old `provisioned_remote`, `remote_runtime_requirements`, `provider_id`, or byte-based volume contracts.
- Add a generic cloud-provider abstraction for future providers.
- Change worker contracts except where names must follow generated command or catalog contract changes.
- Change secret storage semantics.
- Add workspace migration logic for old local databases.

## Target Domain Model

Keep the workspace aggregate shape, but make the current runtime variant RunPod-specific:

```rust
pub enum WorkspaceRuntime {
    Runpod(RunpodRuntime),
}

pub struct RunpodRuntime {
    pub placement: RunpodPlacementPlan,
    pub resources: RunpodResources,
}

pub struct RunpodPlacementPlan {
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub volume_size_gb: u64,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}

pub struct RunpodResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}
```

Use RunPod naming where the RunPod API has stable names:

- `data_center_id` maps to RunPod `dataCenterIds`.
- `gpu_type_id` maps to RunPod `gpuTypeIds`.
- `volume_size_gb` maps to network volume `size`.
- `network_volume_id` maps to `networkVolumeId`.
- `template_id` maps to `templateId`.

GPU memory should be represented as `vram_gb` in placement options because the RunPod GraphQL response exposes `memoryInGb`. Network volume sizes should be represented as GB everywhere above the RunPod HTTP payload builder. The previous `bytes_to_runpod_volume_gb` conversion should disappear.

## Workflow Catalog

Replace generic remote runtime requirements with RunPod requirements:

```rust
pub struct WorkflowRevision {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub runpod_runtime_requirements: RunpodRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

pub struct RunpodRuntimeRequirements {
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
}
```

The bundled workflow catalog changes from:

```json
"remote_runtime_requirements": {
  "required_base_volume_size_bytes": 18837849239,
  "provider_requirements": [
    {
      "gpu_cloud_provider_id": "runpod",
      "endpoint_contract": { "...": "..." },
      "provisioner_contract": { "...": "..." }
    }
  ]
}
```

to:

```json
"required_volume_size_gb": 19,
"runpod_runtime_requirements": {
  "endpoint_contract": { "...": "..." },
  "provisioner_contract": { "...": "..." }
}
```

`required_volume_size_gb` is a workflow revision field because it constrains the workspace volume independent of contract image references. Native workspace creation should reject a RunPod placement whose `volume_size_gb` is less than the selected workflow revision's `required_volume_size_gb`.

## Command Contract

Rename the RunPod-specific commands and DTOs:

- `create_workspace` -> `create_runpod_workspace`
- `CreateWorkspaceRequest` -> `CreateRunpodWorkspaceRequest`
- `remotePlacement` -> `placement`
- `RemotePlacementPlanInput` -> `RunpodPlacementPlanInput`
- `RemotePlacementOptionsResponse` -> `RunpodPlacementOptionsResponse`
- `get_provider_placement_options` -> `get_runpod_placement_options`
- remove `GetProviderPlacementOptionsRequest`
- remove `GpuCloudProviderIdDto`

The target create request:

```rust
pub struct CreateRunpodWorkspaceRequest {
    pub workflow_preset_id: String,
    pub placement: RunpodPlacementPlanInput,
}
```

The placement-options command takes no request because there is no provider selector.

Workspace read commands can remain workspace-oriented:

- `get_workspace_catalog`
- `provision_workspace`
- `cleanup_workspace`
- `delete_workspace`
- lifecycle operation read commands

Those commands act on workspace identity and lifecycle state, not on provider selection. Their response payloads should expose `runtimeType: "runpod"` for the current runtime variant.

## Application Service Boundary

Rename the application service from the generic provisioned-remote concept to `RunpodRuntimeService`. The service owns runtime lifecycle behavior, while `workspace_catalog` owns workspace aggregate persistence.

The service owns:

- RunPod placement options.
- RunPod workspace creation.
- RunPod lifecycle operation startup.
- Marking stale RunPod lifecycle operations on app boot.

The service should hold one concrete RunPod client boundary instead of a provider registry:

```rust
pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options(...);
    fn create_network_volume(...);
    fn delete_network_volume(...);
    fn start_provisioner_pod(...);
    fn terminate_provisioner_pod(...);
    fn get_provisioner_status(...);
    fn create_serverless_endpoint(...);
    fn delete_serverless_endpoint(...);
    fn delete_template(...);
}
```

This trait remains useful for tests because it isolates external I/O. It is not a cloud-provider abstraction; its methods and parameter types are RunPod-specific.

## Provisioning Flow

Provisioning resolves the persisted `Workspace.workflow` reference to `WorkflowPresetResolved`, then resolves `runpod_runtime_requirements` against bundled endpoint and provisioner contract catalogs.

The RunPod provisioning sequence is:

1. Create network volume using `placement.data_center_id` and `placement.volume_size_gb`.
2. Persist `resources.network_volume_id`.
3. Start CPU provisioner pod with the network volume mounted at the RunPod provisioner mount path.
4. Persist `resources.provisioner_pod_id`.
5. Poll the provisioner worker until it succeeds or fails.
6. Terminate the provisioner pod.
7. Clear `resources.provisioner_pod_id`.
8. Create the Serverless template for the endpoint worker.
9. Create the Serverless endpoint using the new `template_id`, `network_volume_id`, `data_center_id`, and `gpu_type_id`.
10. Persist both `resources.template_id` and `resources.endpoint_id`.
11. Mark the workspace ready.

`create_serverless_endpoint` should return both endpoint id and template id. If endpoint creation fails after template creation, the RunPod client should delete the template before returning the error, matching current behavior.

## Cleanup And Delete Flow

Cleanup and delete should use persisted RunPod resource IDs directly:

1. Delete endpoint if `endpoint_id` exists.
2. Clear `endpoint_id` after success or not-found.
3. Delete template if `template_id` exists.
4. Clear `template_id` after success or not-found.
5. Terminate provisioner pod if `provisioner_pod_id` exists.
6. Clear `provisioner_pod_id` after success or not-found.
7. Delete network volume if `network_volume_id` exists.
8. Clear `network_volume_id` after success or not-found.

The current `delete_endpoint_and_template(endpoint_id)` lookup path should be removed. Persisting `template_id` makes the cleanup contract explicit and avoids recovering a required resource id from a secondary RunPod endpoint details request.

If a workspace has an endpoint id without a template id, treat the runtime as corrupt for the current schema. Do not add fallback lookup behavior for old rows.

## Persistence

Replace the workspace runtime schema directly:

- remove `provider_id`
- keep `runtime_type`
- use `runtime_type = "runpod"` for RunPod workspaces
- keep `runtime_json` for runtime-specific placement and resources

Because the project is pre-v1, bootstrap should validate the current schema directly and should not include migration compatibility for old `provider_id` or `runtime_type = "provisioned_remote"` rows.

`RunpodRuntime` serialization should contain:

```json
{
  "placement": {
    "data_center_id": "EU-RO-1",
    "gpu_type_id": "NVIDIA GeForce RTX 4090",
    "volume_size_gb": 19,
    "keep_alive_limits": null
  },
  "resources": {
    "network_volume_id": null,
    "provisioner_pod_id": null,
    "endpoint_id": null,
    "template_id": null
  }
}
```

## Lifecycle Payloads And Errors

Rename lifecycle payload/runtime tags from `provisioned_remote` to `runpod`.

Step names may stay resource-oriented, but should use RunPod names where they refer to concrete resources:

- `CreateNetworkVolume`
- `StartProvisionerPod`
- `PollProvisioner`
- `TerminateProvisionerPod`
- `CreateEndpoint`
- `DeleteEndpoint`
- `DeleteTemplate`
- `DeleteNetworkVolume`
- `DeleteLocalWorkspace`

Error names should remove provider-adapter language:

- `ProviderAdapterUnavailable` should disappear.
- `ProviderSecretUnavailable` should become `RunpodSecretUnavailable`.
- `ProviderApiFailed` should become `RunpodApiFailed`.
- remote not-found errors should use RunPod resource names: `NetworkVolumeNotFound`, `ProvisionerPodNotFound`, `EndpointNotFound`, `TemplateNotFound`.

The command error layer can keep frontend-safe high-level codes, but provider-shaped internal names should not remain.

## Frontend Impact

Regenerate `src/generated/commands.ts` after native command changes.

Update diagnostics/Home page calls:

- remove `providerId: "runpod"` from placement options.
- rename `getProviderPlacementOptions` call usage to `getRunpodPlacementOptions`.
- rename `createWorkspace` sample usage to `createRunpodWorkspace`.
- change create input from `remotePlacement` with byte size to `placement` with GB size.

React should not encode provider decisions. It only submits the selected RunPod placement values required by the RunPod command contract.

## Testing

Update native tests for:

- workflow catalog validation accepts `required_volume_size_gb` and `runpod_runtime_requirements`.
- workflow catalog validation rejects zero `required_volume_size_gb`.
- workflow catalog validation rejects missing endpoint or provisioner contract references.
- RunPod placement mapping preserves `memoryInGb` as `vram_gb`.
- create RunPod workspace rejects volume sizes below the workflow revision requirement.
- create RunPod workspace persists `WorkspaceRuntime::Runpod`.
- SQLite schema has no `provider_id` column.
- SQLite runtime round trips `runtime_type = "runpod"`.
- provisioning persists `network_volume_id`, `provisioner_pod_id`, `endpoint_id`, and `template_id` at the correct lifecycle stages.
- cleanup/delete clear endpoint, template, provisioner pod, and network volume resources independently.
- cleanup/delete treat endpoint-without-template as corrupt runtime state.
- generated command types expose RunPod-specific command and DTO names.

Run full native verification after implementation:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Because generated command bindings and frontend diagnostics change, also run:

```bash
bun run codegen:commands
bun run build
bun run lint
```

## Spec Review Checklist

- No generic cloud-provider selection remains in the target model.
- No byte-based volume size remains above the RunPod HTTP payload boundary.
- No compatibility path is specified for old runtime or workflow catalog rows.
- `create_workspace` is renamed because the command creates a RunPod workspace.
- `template_id` is persisted and used for cleanup.
