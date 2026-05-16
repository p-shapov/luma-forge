## Context

Workspace Provisioning is a sync-driven native workflow. Each sync call derives one action from durable Workspace metadata, performs at most one provider or worker activity, persists any resulting authoritative metadata, and returns generated binding-safe progress to React.

The current implementation already follows this model for many happy-path checkpoints, but several provider mutation paths still treat `ProviderOperationIndeterminate` as a retryable command error after a create request may have reached RunPod. Because the next sync sees no persisted snapshot, it can issue a second create for the same logical Workspace resource.

The current implementation also lets `ProviderResourceNotFound` escape from required resource refresh calls as a command error. That leaves the Workspace in `provisioning` with stale metadata instead of recording a durable failure that tells the user cleanup or inspection is required.

## Goals / Non-Goals

**Goals:**

- Prevent blind duplicate RunPod resource creation after indeterminate create outcomes.
- Adopt exactly one safe Workspace-correlated provider resource when discovery can prove it exists.
- Persist structured Workspace failure detail when Native cannot safely continue.
- Preserve known cleanup metadata already represented in Workspace metadata.
- Delete stale Provisioner Worker bearer tokens even when active pod metadata is missing.
- Make cancellation conflict with an active sync explicit and retryable instead of presenting false success.
- Preserve existing command DTOs and generated frontend response shapes.

**Non-Goals:**

- Add a full public Workspace Resource Cleanup command.
- Add a new Workspace data model for storing multiple orphan provider resource identifiers.
- Repair already-failed Workspaces automatically.
- Change provider setup, workspace setup, prepared runtime, or Endpoint Worker behavior.
- Expose RunPod transport payloads or provider-specific raw errors outside the provider boundary.

## Decisions

### Fail closed at unsafe create boundaries

All non-idempotent provider create calls will handle `ProviderOperationIndeterminate` explicitly. The service will not let that error bubble as a retryable command error when the provider may have accepted a create request.

The sync step should follow this shape:

```text
create resource
  ├─ success -> persist snapshot, then continue normal status handling
  ├─ indeterminate -> discover/adopt if exactly one safe match exists
  │                  otherwise persist failed Workspace state
  └─ other error -> keep existing retry/request-rejection command behavior
```

Rationale: a command-level retry is only safe when repeating the same operation cannot create a duplicate paid resource. Indeterminate create is not in that category.

Alternative considered: mark every indeterminate create failed immediately without discovery. That is simpler, and already avoids duplicates, but it misses the chance to retain cleanup metadata and resume safely when RunPod exposes a clear Workspace-correlated resource.

### Keep resource discovery provider-neutral at the provisioning boundary

Extend `ProviderProvisioningGateway` with discovery methods for resources that need adoption after a lost create response:

- network volume by Workspace-derived name and placement context
- endpoint template by Workspace-derived name and expected template properties
- serverless endpoint by Workspace-derived name plus template, volume, and placement context

Provisioning service code should consume provider-neutral observations. RunPod REST list endpoints, filtering fields, DTO changes, and response parsing remain in `provider/runpod` and are adapted in `provider/registry`.

Rationale: Workspace Provisioning should reason about durable workflow state, not RunPod response shapes.

Alternative considered: use RunPod-specific helpers directly from the provisioning service. That would be faster locally but violates the existing native-boundary direction and makes future provider support harder.

### Treat exactly-one discovery as adoption, zero or many as unsafe

Discovery after an indeterminate create has three outcomes:

```text
0 matches      -> failed, no new cleanup snapshot
1 safe match   -> persist snapshot and continue normal sync
2+ matches     -> failed, preserve existing metadata, do not choose arbitrarily
```

For provisioning pods, the existing pre-create discovery by stable pod name and network volume id should be reused after indeterminate create. If exactly one matching pod exists, persist it as the active pod snapshot. If no pod or multiple pods exist, mark failed.

For volumes, templates, and endpoints, add discovery where RunPod provides enough list data to match the deterministic name and expected properties. If a RunPod list response does not provide enough fields to prove ownership and compatibility, the provider adapter should return zero safe matches rather than guessing.

Rationale: adopting the wrong resource is worse than failing closed. Choosing among multiple matching resources can bind the Workspace to an arbitrary duplicate and leave the others unmanaged.

Alternative considered: adopt the newest or first returned matching resource. That hides ambiguity and can still leak resources.

### Convert missing tracked resources into durable provider-resource failures

Refresh paths for persisted snapshots should handle `ProviderResourceNotFound` locally:

- missing volume -> `ProviderResourceMissing`, phase `CreatingVolume`
- missing active provisioning pod -> `ProviderResourceMissing`, phase `StartingProvisioningPod`
- missing endpoint template -> `ProviderResourceMissing`, phase `CreatingEndpointTemplate`
- missing serverless endpoint -> `ProviderResourceMissing`, phase `CreatingEndpoint`

The Workspace should move to `failed`, retain known snapshots, and return failed progress. Already-missing resources should still be tolerated during cleanup deletion.

Rationale: a missing required tracked resource means the durable workflow checkpoint is no longer valid. Returning only a command error leaves the sync loop unable to progress and gives the user no durable recovery state.

Alternative considered: clear the missing snapshot and recreate. That risks recreating resources after an external deletion without user intent and can hide provider-side drift.

### Delete worker tokens independently from active pod snapshots

Shared cleanup should attempt to delete the per-workspace Provisioner Worker bearer token for every Workspace cleanup request, regardless of whether an active pod snapshot exists.

Rationale: token creation happens before pod creation. If pod creation is indeterminate after the token is written but before a pod snapshot is persisted, snapshot-gated token deletion leaves stale worker credentials in keyring.

Alternative considered: delete the token only in the pod-create indeterminate handler. That fixes one path but keeps cleanup behavior inconsistent for future failure modes.

### Return retryable conflict when cancel cannot acquire the coordinator

`cancel` should return `ProviderOperationConflict` when another sync currently owns the same Workspace. It should not return unchanged provisioning metadata as a successful cancellation.

React already logs and toasts command errors. If needed, the frontend can leave or restart auto-sync based on the retryable conflict, but it must not show the cancellation success copy for this case.

Rationale: cancellation is destructive cleanup. If cleanup did not run, the native command must not report success.

Alternative considered: wait for the active sync guard and then cancel. That can create long-running command behavior and complicate Tauri command cancellation. A retryable conflict is simpler and honest.

## Risks / Trade-offs

- RunPod list endpoints may not expose enough fields for exact discovery -> fail closed and preserve existing metadata rather than adopting uncertain resources.
- Discovery adds provider API calls after indeterminate creates -> only happens on exceptional paths and prevents more expensive duplicate resources.
- Multiple matching resources cannot all be persisted in the current Workspace model -> fail closed and avoid additional creation; full orphan cleanup remains future cleanup-command work.
- Returning conflict from cancel can require a second user action or retry -> better than false cancellation success and resource leakage.
- Deleting tokens unconditionally can surface keyring errors in cleanup paths that previously skipped token deletion -> cleanup should report failure when required local cleanup cannot be confirmed.

## Migration Plan

No persisted schema migration is expected. Existing Workspace metadata remains compatible.

Implementation can be rolled out as a native behavior change:

1. Add provider-neutral discovery contracts and RunPod provider adapters.
2. Update provisioning sync error handling for indeterminate creates and missing refresh resources.
3. Update shared cleanup token deletion and cancellation conflict behavior.
4. Regenerate command bindings only if error metadata changes require it; response shapes should remain stable.
5. Run backend tests and clippy/fmt for native changes, plus frontend build/lint if React behavior is adjusted.

Rollback is reverting the change. No new durable fields are introduced, so rollback does not require data conversion.

## Open Questions

- Which RunPod REST list endpoints expose enough fields to safely discover network volumes, templates, and endpoints by deterministic name and expected properties?
- Should cancellation conflict leave frontend auto-sync stopped, restart it, or simply rely on the user retrying cancel after the in-flight sync completes?
