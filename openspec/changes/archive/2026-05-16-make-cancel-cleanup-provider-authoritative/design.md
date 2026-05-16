## Context

Workspace Provisioning cancellation currently routes through shared known-resource cleanup. That helper attempts to cancel the active Provisioner Worker job before deleting RunPod resources, and records the first error across the whole cleanup sequence.

This means a worker-level failure can make cancellation fail even when Native successfully deletes the serverless endpoint, endpoint template, provisioning pod, persistent volume, and local worker token. That does not match the ownership model: Native owns provider resources and durable Workspace state, while the Provisioner Worker only performs temporary preparation work inside the provisioning pod.

## Goals / Non-Goals

**Goals:**

- Make successful provider resource cleanup sufficient for successful Workspace Provisioning cancellation.
- Remove the worker `/cancel` call from destructive Workspace Provisioning cancellation.
- Remove `POST /cancel` from the Provisioner Worker HTTP API.
- Avoid depending on the Provisioner Worker API while Native is already tearing down the pod that hosts it.
- Preserve failure behavior when known Provider Resources cannot be deleted or confirmed missing.
- Preserve secret safety for Provider API Keys and Provisioner Worker bearer tokens.
- Keep the frontend command contract unchanged.

**Non-Goals:**

- Do not introduce a new Workspace Resource Cleanup UI flow.
- Do not change provisioning sync behavior before the user requests cancellation.
- Do not add provider-specific retry orchestration beyond the existing cleanup behavior.

## Decisions

1. Provider deletion is the authoritative cancellation stop signal.

   Native should delete the RunPod provisioning pod instead of relying on the worker `/cancel` endpoint. Terminating the pod stops the worker process and prevents further preparation work. This aligns cancellation with the resource owner: Native can validate provider resource deletion, but it cannot rely on a temporary in-pod service being reachable during teardown.

   Alternative considered: keep `/cancel` as a required cleanup step. This preserves graceful worker shutdown but incorrectly treats an unreachable temporary service as a leaked provider resource.

2. Destructive Workspace cancellation does not call worker `/cancel`.

   The cleanup implementation should skip the worker `/cancel` call entirely for Workspace Provisioning cancellation. The cleanup result should be based on provider resource deletion and required local cleanup. This keeps cancellation deterministic and avoids network calls to a service that may be unavailable precisely because its pod is being torn down.

   Alternative considered: keep `/cancel` as best-effort and ignore its errors. That still adds unnecessary latency and ambiguity to a destructive teardown path. If graceful worker cancellation is needed later, it should belong to a non-destructive flow that keeps provider resources alive.

3. The Provisioner Worker no longer exposes `POST /cancel`.

   Once Native stops calling worker `/cancel`, keeping the endpoint would preserve an unused public API and keep misleading tests/docs around graceful cancellation. Removing the endpoint makes the worker contract match the new cancellation model: Native cancels by terminating the provider pod.

   Alternative considered: keep the worker endpoint for possible future non-destructive cancellation. That future flow does not exist in v1, so keeping the endpoint now increases API surface without a caller.

4. Cleanup failure metadata remains conservative.

   If Native cannot delete a known Provider Resource or cannot complete required local cleanup after provider resources are no longer needed, the Workspace should still become `failed` with `cancellation_cleanup_failed` and retain known cleanup metadata.

   Alternative considered: always return to `draft` after attempting deletes. That risks hiding leaked provider resources and losing metadata needed for recovery.

## Risks / Trade-offs

- Worker does not receive graceful cancellation before pod deletion -> Acceptable because cancellation explicitly tears down the pod and volume, and the worker must not own durable provider resources outside Native metadata.
- Provider delete request may be indeterminate -> Keep existing failure behavior and preserve cleanup metadata for manual recovery.
- Token deletion failure after successful provider cleanup can still fail cancellation -> This preserves local secret cleanup guarantees; if this becomes noisy, handle it as a separate local recovery policy change.
- Removing worker `/cancel` removes graceful in-process cancellation for direct worker clients -> Acceptable because the supported v1 cancellation path is provider pod termination owned by Native.
