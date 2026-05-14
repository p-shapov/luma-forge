## Why

LumaForge needs a minimal RunPod Endpoint Worker before Workspace Provisioning can create a persistent RunPod runtime entry point for generated images. The worker should prove the provider-specific runtime boundary and first generation path without forcing the provisioning flow to spend user GPU time on a post-provisioning test job.

## What Changes

- Add a minimal RunPod Endpoint Worker runtime under the worker boundary that can run behind a RunPod Serverless endpoint.
- Define a narrow v1 generation contract that accepts an execution type and one text prompt, supports the current `t2i` path, and returns one generated image.
- Keep full custom-node communication, generalized ComfyUI graph editing, advanced job management, and multi-provider endpoint serving outside this change.
- Keep the RunPod Endpoint Worker separate from the Provisioner Worker; it must assume ComfyUI, models, custom nodes, and workflow assets already exist in the mounted prepared environment.
- Clarify that provisioning readiness must not invoke generation or submit paid endpoint jobs.
- Prepare Native build configuration for provider-specific Endpoint Worker deployment artifacts while keeping the Provisioner Worker artifact provider-neutral.

## Capabilities

### New Capabilities

- `endpoint-worker`: Defines the minimal runtime API and behavior for the RunPod Endpoint Worker that bridges RunPod endpoint invocations to a prepared ComfyUI environment.

### Modified Capabilities

- `native-build-configuration`: Endpoint Worker deployment artifacts become provider-specific build configuration, with RunPod configured for v1 while the Provisioner Worker artifact remains provider-neutral.

## Impact

- Affected worker code: new RunPod Endpoint Worker package, runtime entrypoint, request/response schemas, ComfyUI adapter boundary, and tests.
- Affected native code: build-time configuration parsing and documentation for RunPod Endpoint Worker image refs and ports.
- Affected specs/docs: Endpoint Worker requirements, native build configuration requirements, Workspace Provisioning clarification that readiness does not run generation.
- No React generation UI, Workspace Provisioning implementation, RunPod endpoint provisioning implementation, or full custom-node protocol is included in this change.
