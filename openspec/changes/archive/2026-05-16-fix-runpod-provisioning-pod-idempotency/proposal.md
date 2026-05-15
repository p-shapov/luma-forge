## Why

Workspace Provisioning can create orphan RunPod provisioning pods when RunPod accepts a pod creation request but the returned pod payload does not contain every field currently required by LumaForge before persisting the active pod snapshot. A retry then sees a ready volume with no active pod snapshot and issues another non-idempotent create request.

RunPod HTTP-exposed pods also do not require direct `publicIp` and `portMappings` to be usable. The current parser treats missing direct TCP metadata as invalid even though the correct HTTP access path is the RunPod proxy URL derived from pod id and internal port.

## What Changes

- Make RunPod provisioning pod creation persist a durable active pod snapshot as soon as the provider resource id is known.
- Derive RunPod provisioning pod data center and GPU metadata from the requested placement when RunPod does not echo those fields in the create/get payload.
- Derive Provisioner Worker status URLs for HTTP-exposed RunPod pods using the RunPod proxy URL format.
- Add provider-resource adoption before creating a provisioning pod when local state has a ready volume but no active pod snapshot.
- Mark provisioning failed with cleanup metadata instead of creating another pod when provider discovery finds multiple matching Workspace-correlated pods.
- Preserve secret safety: no provider payloads, bearer tokens, or API keys are exposed in command responses, logs, diagnostics, or Workspace metadata.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-provisioning`: Provisioning pod creation and recovery must be idempotent across invalid provider payloads, lost local snapshots, and retries, and RunPod HTTP pod access must use provider-correct status URL derivation.

## Impact

- Affects `src-tauri/src/workspace_provisioning`, especially temporary provisioning pod sync, failure handling, and tests.
- Affects `src-tauri/src/provider/runpod` and `src-tauri/src/provider/registry` response mapping for RunPod pod observations.
- May add a provider gateway discovery/list operation for Workspace-correlated RunPod pods.
- May affect Workspace Provisioning failure progress when duplicate provider-side pods already exist.
- Does not change the frontend command contract, generated response shape, provider API key handling, endpoint provisioning semantics, or the requirement that React treats Native responses as authoritative.
