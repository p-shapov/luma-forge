## Context

LumaForge v1 provisions every workspace by first creating a RunPod network volume, then using that volume for the provisioning pod and serverless endpoint. RunPod's placement inventory can report GPU availability for datacenters that do not support network storage, so the current provider inventory can offer placements that cannot pass the first provisioning mutation.

The current Workspace Setup contract returns provider inventory as datacenters with GPU options and does not expose a storage-support capability per datacenter. Workspace creation intentionally does not revalidate live GPU or datacenter availability, so the best point to prevent known-impossible placement choices is the live provider placement options response.

## Goals / Non-Goals

**Goals:**

- Ensure RunPod placement options only include datacenters that can support the persistent network volume required by current provisioning.
- Keep RunPod-specific storage capability handling inside native provider infrastructure.
- Avoid exposing Provider API Keys, raw provider responses, or provider-specific error details.
- Preserve the current command response shape unless implementation discovers a stronger need for an explicit domain capability.
- Cover the behavior with focused native tests.
- Prevent the user from creating or starting provisioning for placements that loaded inventory reports as unavailable.

**Non-Goals:**

- Do not create Provider Resources during Workspace Setup.
- Do not add no-volume or ephemeral workspace provisioning modes.
- Do not revalidate live datacenter/GPU availability when creating a Workspace from a previously selected placement.
- Do not add frontend-specific RunPod storage rules.

## Decisions

1. Filter storage-unsupported datacenters in the RunPod provider client mapping.

   The RunPod GraphQL response already has provider-specific datacenter fields. The provider client should request `storageSupport` and treat `storageSupport != true` as not eligible for LumaForge placement options. This keeps RunPod API quirks out of React and out of provider-neutral Workspace Setup service logic.

   Alternative considered: expose `storageSupport` in the domain `Datacenter` and let React disable choices. This would make the UI aware of a provider-specific capability that is mandatory for all current workflows and would still leave downstream code to defend against invalid selections.

2. Preserve the provider inventory contract for v1.

   The current domain inventory can continue representing only datacenters that are eligible for the app's provisioning model. A new `supports_persistent_storage_volume` field is unnecessary while every returned RunPod datacenter must support network volumes.

   Alternative considered: add a generic capability field now. That is more explicit, but it expands generated bindings and frontend handling before there is a real no-volume workflow that needs to display unsupported datacenters.

3. Keep Workspace creation live-availability behavior unchanged.

   Workspace creation should still validate structure, catalog compatibility, storage size range, and endpoint keep-alive range without requiring the selected datacenter/GPU to still appear in live inventory. The filtering only affects fresh placement options returned by `get_provider_placement_options`.

   Alternative considered: reject workspace creation when the selected datacenter no longer appears in the latest provider inventory. That would require an additional live provider call during creation, weaken the existing separation between setup metadata and provider-owned availability, and introduce new failure modes unrelated to this bug.

4. Treat a zero availability score as a frontend blocking condition.

   The provider inventory already carries a normalized `availability_score`. React should keep unavailable GPUs visible so the user can understand why a placement is blocked, but it should disable workspace creation for a selected zero-score GPU and disable provisioning start when the loaded placement options show the selected workspace GPU is unavailable. Sync and cancel remain available because those operations inspect or unwind existing provider state rather than starting new capacity.

   Alternative considered: filter zero-score GPUs in the native provider mapping. That would hide useful placement diagnostics and conflict with the decision to display availability in the UI.

## Risks / Trade-offs

- Hidden compute-only datacenters -> Acceptable for v1 because LumaForge cannot provision without a network volume.
- RunPod changes or omits `storageSupport` -> Treat missing/null as unsupported to fail closed; tests should cover this.
- Empty placement inventory in regions where storage support is unavailable -> The UI will show fewer or no choices, which is preferable to creating an impossible workspace.
- Existing draft workspaces with unsupported datacenters remain persisted -> This change prevents new selections but does not migrate or rewrite existing workspace records; provisioning may still fail for already-created drafts until the user creates a new workspace with an eligible placement.
- Availability can change after placement options are fetched -> The UI guard prevents known-unavailable starts, but RunPod capacity can still change between inventory fetch and provisioning; native provisioning must continue handling provider failures.

## Migration Plan

No data migration is required. The change affects newly fetched placement options only.

Rollback is to stop requesting/filtering by `storageSupport`, restoring the previous broader inventory behavior.

## Open Questions

- If future workflows do not require persistent network volumes, should provider inventory expose per-datacenter storage capability instead of filtering? This is out of scope for the current v1 provisioning model.
