## Context

LumaForge currently treats the Runtime Catalog as both a runtime selector and a runtime compatibility proof. A Workspace snapshot carries contract id/version, implementation revision, worker image refs, runtime metadata, image layout metadata, manifest compatibility, and overlay policy. Native code validates and persists that full snapshot, Workspace Provisioning injects runtime identity values into RunPod resources, and the workers validate the request and mounted workspace against image-baked metadata.

The app is not in production, has one developer, and does not need runtime checks for expected Python, ComfyUI revision, overlay policy, dependency records, or metadata files. Worker build and release data still matters, but it belongs to worker recipes and Docker build inputs rather than the app Runtime Catalog.

## Goals / Non-Goals

**Goals:**

- Reduce the app Runtime Catalog to contract families with ids and nested versioned image refs.
- Use runtime contract id and version as the Workflow Preset runtime requirement.
- Persist a minimal Workspace runtime snapshot containing contract id, contract version, and selected worker image refs.
- Remove implementation revisions, catalog display metadata, runtime metadata, image metadata, runtime identity provider env vars, and worker identity validation.
- Keep fixed runtime layout and overlay behavior as worker/native constants instead of catalog data.
- Keep worker release/build recipe data on the worker side.

**Non-Goals:**

- Preserve compatibility with existing persisted Workspace records.
- Support rollback through historical Runtime Catalog revisions.
- Add catalog schema versioning.
- Add multi-provider runtime selection or provider-specific catalog profiles.
- Change GPU placement, provider inventory, provider secrets, or generation APIs beyond removing runtime identity plumbing.

## Decisions

1. The Runtime Catalog shape becomes `{"contracts":[{"id","revisions":[{"version","provisioner_image_ref","endpoint_image_ref"}]}]}`.

   Rationale: the app needs to map a named runtime contract family and version to the two immutable images used during provisioning. Contract id remains useful as the stable runtime family key, while implementation revisions remain unnecessary without production rollback or historical catalog entries.

   Alternative considered: keep flat `contracts[]` entries keyed only by version. Rejected because preserving `id` keeps future multi-family runtime selection explicit without restoring implementation-revision metadata.

2. Workflow Presets require `runtime_contract: { id, version }`.

   Rationale: Workflow Presets still need to state which runtime family and compatibility version they require, but they no longer select an implementation revision.

   Alternative considered: use separate scalar fields. Rejected because a nested value object makes it clear that id and version are one runtime contract reference.

3. Workspace snapshots persist only `contract_id`, `contract_version`, `provisioner_image_ref`, and `endpoint_image_ref`.

   Rationale: Workspaces need enough data to remain pinned to the contract family, version, and image refs selected at draft creation. They do not need runtime metadata, image layout metadata, implementation revision, or validation metadata.

   Alternative considered: resolve from the current bundled catalog at provisioning time. Rejected because a later app build could silently retarget existing Draft Workspaces.

4. Native provisioning stops injecting runtime identity env vars.

   Rationale: `LUMA_FORGE_IMAGE_RUNTIME_ROOT`, `LUMA_FORGE_RUNTIME_CONTRACT_ID`, `LUMA_FORGE_RUNTIME_CONTRACT_VERSION`, `LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION`, `LUMA_FORGE_ENDPOINT_IMAGE_REF`, and `LUMA_FORGE_PROVISIONER_IMAGE_REF` existed to support identity validation and template matching. With runtime validation removed, native should create resources from selected image refs and inject only operational secrets/configuration.

   Alternative considered: keep endpoint env vars for template discovery. Rejected because template matching can rely on provider resource name, selected image ref, mount path, and fixed provider settings; runtime identity env values are no longer authoritative behavior.

5. Worker runtime layout and overlay policy become fixed worker code constants.

   Rationale: current values are fixed in practice: image runtime root `/opt/luma-forge/runtime`, image Python `.venv/bin/python`, image ComfyUI `ComfyUI`, workspace overlay `.luma-forge/python-overlay`, overlay-first import behavior, protected packages `torch`, `torchvision`, `torchaudio`, and prefix `nvidia-`.

   Alternative considered: keep them catalog-configurable. Rejected because there is no current product choice, and configurability adds validation and migration surface without user value.

6. Worker deployment keeps recipe compatibility data outside the app catalog.

   Rationale: Python version, platform, ComfyUI revision, PyTorch index/packages, and base requirements are needed to build images. They no longer need to be embedded in or compared against `bundled/runtime-catalog.json`.

   Alternative considered: keep `runtime_compatibility` as ignored JSON in the app catalog. Rejected because ignored app data causes drift and makes the contract look broader than it is.

## Risks / Trade-offs

- Runtime mistakes move from app/runtime validation to build/release discipline → Mitigate by keeping worker package tests, Docker build tests, and image smoke tests focused on fixed runtime layout.
- Existing Workspace records break after model changes → Accepted because the app is not in production and no backward compatibility is required.
- Removing implementation revisions removes catalog rollback semantics → Accepted because no production rollback workflow is needed; rollback can be handled by publishing a new catalog image ref for the same contract id/version during development.
- Endpoint template discovery may become less strict without runtime env matching → Mitigate by matching provider resource name, endpoint image ref, mount path, and provider-owned template settings.
- Release tooling must stop relying on old catalog compatibility fields → Mitigate by making runtime recipes the source of worker build compatibility data and simplifying catalog updates to id/version/image-ref upserts.
