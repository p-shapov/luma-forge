## Context

The current Workspace Provisioning implementation creates a RunPod provisioning pod after the Workspace volume is ready, then attempts to convert the RunPod response into an active Provisioning Pod snapshot. The observed failure shows RunPod can create the pod while returning an HTTP-pod payload with no `dataCenterId`, no `gpuTypeId`, empty `publicIp`, and no `portMappings`.

That payload is valid enough for RunPod's HTTP proxy model, but LumaForge currently treats it as invalid before persisting the pod id. Because durable Workspace state still says "ready volume, no active pod", the next sync repeats the non-idempotent create request and RunPod creates another pod with the same name.

## Goals / Non-Goals

**Goals:**

- Prevent blind duplicate RunPod provisioning pod creation after a successful provider-side create.
- Persist cleanup-capable provisioning pod metadata as soon as a RunPod pod id is known.
- Support RunPod HTTP-exposed provisioning pods by deriving the worker status URL from pod id and internal HTTP port.
- Adopt exactly one existing Workspace-correlated provisioning pod before creating a new pod when local state lacks an active pod snapshot.
- Fail closed with cleanup metadata or an inspect/cleanup recovery path when multiple matching pods exist.

**Non-Goals:**

- Deleting the currently orphaned pods as part of the proposal.
- Changing the frontend command contract.
- Moving provisioning orchestration into React.
- Introducing provider-specific diagnostics or raw provider payloads into command responses or logs.
- Supporting non-RunPod providers in v1.

## Decisions

### Treat pod id as the minimum durable checkpoint

RunPod provisioning pod creation SHALL be considered locally checkpointable once the provider pod id is known. The active pod snapshot can use request-derived data for selected data center and GPU when RunPod omits those fields, and can use a provider-derived status URL for the worker.

Rationale: cleanup safety depends first on knowing the provider resource id. Requiring all observation fields before persisting the pod id creates the orphan window.

Alternative considered: keep rejecting incomplete create responses and rely on users to manually clean provider resources. That violates provisioning idempotency and makes repeated sync unsafe.

### Use RunPod HTTP proxy URLs for HTTP ports

For a provisioning pod exposing `<port>/http`, the RunPod adapter SHALL derive the Provisioner Worker status URL as:

```text
https://<pod-id>-<port>.proxy.runpod.net/status
```

Direct `publicIp` plus `portMappings` remains appropriate for TCP exposure, but HTTP pods do not need it.

Rationale: RunPod documents HTTP pod access through the proxy URL, while direct public IP and mapped port are the TCP access model. The Provisioner Worker uses HTTP, so the proxy URL is the provider-correct address.

Alternative considered: change the provisioning pod to expose `8000/tcp`. That would require relying on direct TCP port mappings and would not use RunPod's HTTP proxy semantics.

### Make provider pod observation contextual

The provider registry boundary should map RunPod pod responses with request context when available. Create calls know the selected data center, selected GPU, image, volume id, mount path, and requested port. If RunPod omits some of those values in the response, the observation can safely carry request-derived values as long as the pod id is present and the response is otherwise for the created pod.

Rationale: provider create responses are often less complete than later resource observations. Contextual mapping keeps provider quirks inside the provider boundary without weakening domain invariants.

Alternative considered: poll `get_pod` until RunPod returns all fields before persisting. In the observed case `get_pod` still omitted the same fields, and waiting does not solve duplicate creation after a command error.

### Add adoption before creation

Before creating a provisioning pod when the Workspace has a ready volume and no active pod snapshot, the Native Layer SHALL search for existing RunPod pods correlated to the Workspace. The safe match criteria should include the stable Workspace-derived pod name and mounted network volume id; image and port may be additional checks.

If exactly one live matching pod exists, Native SHALL adopt it by persisting an active pod snapshot and continue normal sync. If more than one live matching pod exists, Native SHALL mark the Workspace failed with cleanup/inspection recovery metadata and MUST NOT create another pod.

Rationale: adoption closes the existing orphan window and crash/retry cases where a pod exists but local state did not capture it.

Alternative considered: rely only on local snapshots. That cannot recover from a lost successful create response or from the current orphan state.

### Preserve current secret boundaries

Provisioner Worker bearer tokens remain in keyring and are never written into Workspace metadata. Adoption can persist a status URL, resource id, selected placement values, and provider status, but not remote environment values or raw provider payloads.

Rationale: the recovery logic must not trade idempotency for secret exposure.

Alternative considered: read the pod environment from RunPod and persist token-related diagnostics. That is unnecessary for provisioning and violates existing secret handling constraints.

## Risks / Trade-offs

- Adoption may select the wrong pod if name-only matching is used -> Require at least Workspace-derived name plus network volume id before adopting.
- Multiple already-created matching pods block progress -> Fail closed and retain/derive enough metadata for cleanup rather than selecting arbitrarily.
- The HTTP proxy may be reachable before the worker service is ready -> Persist the pod snapshot and let worker status calls return retryable/unavailable progress until the service responds.
- RunPod may later change proxy URL format -> Keep URL derivation isolated in the RunPod adapter and covered by mapper tests.
- Existing orphan pods may have stale bearer tokens -> Adoption should only make provider cleanup safe; worker calls may still fail unauthorized and should drive failure/cleanup rather than another create.
