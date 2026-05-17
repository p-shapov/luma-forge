## Context

The native build currently reads `LUMA_FORGE_PROVISIONER_WORKER_PORT` and `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT` from the real environment or root `.env`, emits them through `cargo:rustc-env`, and exposes them at runtime through `NativeAppConfig`. `NativeAppState` then copies those values into `WorkspaceProvisioningConfig`, where provisioning uses them to create RunPod pods and serverless templates.

This no longer matches the architecture. Worker image refs are already selected from the bundled Runtime Catalog and persisted Workspace runtime implementation snapshots. The remaining port values are not secrets, operator choices, or per-build deployment targets. They are fixed contracts between the native provisioning code, RunPod resource definitions, and worker containers.

There is also a naming problem: `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT=8188` sounds like a RunPod Endpoint Worker API port, but the endpoint worker uses the RunPod serverless handler and starts ComfyUI internally on port `8188`. The value is the internal ComfyUI HTTP port when it is needed by provider resource configuration.

## Goals / Non-Goals

**Goals:**

- Remove worker port values from root `.env`, `.env.example`, and native build-time configuration.
- Remove obsolete root dotenv worker image ref examples because image refs are no longer read from root env configuration.
- Remove the Cargo build-script handoff used only for worker ports.
- Keep fixed worker/provider deployment values inside native provisioning/provider implementation code.
- Make the endpoint-side port meaning explicit as the RunPod endpoint container's internal ComfyUI HTTP port when it remains needed.
- Preserve Runtime Catalog and Workspace snapshot ownership of worker image refs.

**Non-Goals:**

- Do not change worker image selection or Runtime Catalog schema.
- Do not make worker ports user-configurable.
- Do not introduce provider profile/catalog data for fixed RunPod values.
- Do not change the worker container defaults unless implementation discovers a mismatch with existing worker contracts.
- Do not add support for non-RunPod providers.

## Decisions

### Decision: Remove native build configuration for worker ports

The native build should not parse worker ports or emit them via Cargo environment output. Once ports leave build configuration, `src-tauri/build.rs` can return to the Tauri build default unless future build-time configuration is introduced.

Alternative considered: keep build configuration but provide hard-coded defaults. That preserves unnecessary indirection and still implies developers or release environments may safely vary the ports independently from worker images.

### Decision: Treat fixed ports as implementation constants

The Provisioner Worker HTTP port should be represented as a fixed native provisioning/provider implementation value matching the provisioner container contract. It is used to expose the RunPod pod HTTP port and derive the worker status URL.

Alternative considered: add ports to the Runtime Catalog implementation revision. That would overfit catalog data to provider deployment details and make persisted runtime snapshots carry values that are not currently runtime compatibility choices.

### Decision: Model the endpoint-side port by actual meaning

If RunPod serverless template creation still needs a port entry, the value should be named and scoped as the RunPod endpoint container's internal ComfyUI HTTP port. It should not be passed around as a generic `endpoint_worker_port`.

Alternative considered: remove the endpoint template port entirely. This may be correct if RunPod serverless templates do not require exposing the internal ComfyUI port, but it should be verified against current RunPod behavior before deleting the provider request field.

### Decision: Keep `WorkspaceProvisioningConfig` only for values that are genuinely service-level configuration

`WorkspaceProvisioningConfig` should not be a pass-through for fixed provider constants. It may keep values such as the workspace mount path if the implementation still treats them as service coordination values, but fixed RunPod resource details should live closer to the RunPod provider boundary.

Alternative considered: remove `WorkspaceProvisioningConfig` entirely. That is not required for this change and may create unrelated churn.

## Risks / Trade-offs

- RunPod template port semantics are ambiguous -> Verify whether serverless template `ports` is required for this endpoint worker before removing or renaming the field in provider contracts.
- Existing tests may encode `8080` as a configurable fake port -> Update tests to assert fixed contract values where appropriate and keep arbitrary test ports only inside lower-level serialization/unit tests.
- Removing build config changes spec behavior -> Update `native-build-configuration` requirements so future work does not reintroduce `.env` worker ports.
- Provider constants can become hidden magic numbers -> Name constants by domain meaning and keep them near the provider/resource code that uses them.

## Migration Plan

1. Remove obsolete worker variables from root `.env` and `.env.example`.
2. Remove the build-time app config parser and runtime `NativeAppConfig` if no build configuration remains.
3. Replace `WorkspaceProvisioningConfig` port fields with fixed provisioning/provider constants.
4. Rename endpoint-side port concepts to ComfyUI HTTP port if the RunPod template still needs them.
5. Update specs and tests to reflect that native builds no longer require worker port environment values.

Rollback is straightforward: reintroduce the old `.env` variables and build-script handoff if a later provider/runtime requirement proves ports must vary per build. That rollback should be avoided unless the port value becomes a real release-time artifact.

## Open Questions

- Does RunPod serverless template creation require a `ports` entry for this handler-based endpoint worker, or can the endpoint template omit the internal ComfyUI port?
- Should the workspace mount path remain in `WorkspaceProvisioningConfig`, or should it also move fully into the RunPod provider implementation in a separate cleanup?
