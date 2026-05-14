## Context

LumaForge already has a Provisioner Worker that prepares a mounted ComfyUI workspace and a documented Workspace Provisioning flow that creates a Serverless Endpoint after preparation. There is no Endpoint Worker implementation yet, so the future RunPod Serverless endpoint has no concrete runtime image or minimal generation contract to execute.

The Provisioner Worker can remain provider-neutral because it only needs temporary compute, a mounted filesystem, and an HTTP control API. The Endpoint Worker is more provider-shaped: RunPod queue-based Serverless invokes a worker handler through `/run`, `/runsync`, and `/status`, while future providers may use different request routing, scaling, or serving semantics.

Provisioning must not submit a generation job just to prove readiness. The first paid compute invocation should be user-initiated generation, not a hidden post-provisioning smoke test.

## Goals / Non-Goals

**Goals:**

- Add a minimal RunPod-specific Endpoint Worker package and container boundary.
- Support one narrow text-prompt-to-image generation contract selected through the `t2i` execution type.
- Keep the worker runtime secret-safe and UI-safe in responses and logs.
- Keep ComfyUI preparation, downloads, dependency installation, and provider resource lifecycle outside the Endpoint Worker.
- Update Native build configuration so Endpoint Worker deployment artifacts are provider-specific, with RunPod configured for v1.

**Non-Goals:**

- No React generation UI.
- No Workspace Provisioning implementation.
- No generalized custom-node protocol.
- No arbitrary ComfyUI workflow graph editor or user-uploaded workflow execution.
- No image object storage, gallery persistence, streaming progress, cancellation, or webhooks.
- No Vast or other non-RunPod Endpoint Worker deployment.

## Decisions

### Use a separate `workers/runpod-endpoint` package

The RunPod Endpoint Worker will live under `workers/runpod-endpoint` instead of a generic endpoint worker directory or `workers/provisioner`. The two workers have different lifecycles: the Provisioner Worker is temporary setup compute, while the RunPod Endpoint Worker is the persistent runtime entry point behind a RunPod Serverless endpoint.

Alternative considered: extend the Provisioner Worker to also serve generation. That would couple provisioning and runtime behavior, risk leaving setup-only APIs exposed in the persistent endpoint, and contradict the existing invariant that the Provisioner Worker must not be used as the persistent runtime entry point.

### Target RunPod queue-based Serverless for v1

The minimal worker will package a RunPod Serverless handler as the outer provider adapter. RunPod queue endpoints already define job submission and result polling through provider APIs, so the worker does not need to expose its own public HTTP server for v1.

Alternative considered: build a generic HTTP service first. That could fit load-balancing endpoints or other providers later, but it would not match the current v1 RunPod queue endpoint path and would add an extra serving contract before the first generation path works.

### Dispatch generation by execution type

The v1 request includes an execution type and a text prompt. The current bundled catalog has one `t2i` preset, so the RunPod Endpoint Worker only needs to support `t2i` initially. The worker is not created per preset; provider-specific workers should dispatch internally based on execution type and then map the request to the matching prepared ComfyUI workflow.

Alternative considered: build workers per Workflow Preset. That would make each preset easier to hardcode but would create unnecessary image proliferation and make future preset changes more operationally expensive.

### Keep the generation contract intentionally narrow

The v1 request accepts a text prompt for the `t2i` execution type and returns one generated image. The Endpoint Worker owns the mapping from the prompt to the known ComfyUI workflow JSON and the extraction of the expected output image. It does not expose arbitrary graph mutation or custom-node-specific APIs.

Alternative considered: design the full app-to-endpoint-to-ComfyUI protocol now. That is the right long-term direction, but it is a separate scope because it affects workflow editing, custom node contracts, progress events, cancellation, artifact storage, and typed generation parameters.

### Return exactly one inline image for the minimal path

The first worker will return exactly one image as base64 with MIME metadata in the job output. This is a temporary v1 contract; a later endpoint protocol change can replace it with richer artifact handling, multiple outputs, progress, cancellation, or durable storage.

Alternative considered: upload generated images to provider storage or a persistent volume and return URLs. That is better for large outputs and long-lived artifacts, but it adds a storage contract that is not needed to prove the first generation path.

### Treat ComfyUI startup as runtime behavior, not provisioning

The RunPod Endpoint Worker owns the local ComfyUI process by default. It starts the prepared ComfyUI `main.py` lazily before the first valid generation request, waits for `/system_stats`, reuses that process for later jobs in the same warm worker, and terminates the child process during worker shutdown. It must not start ComfyUI separately for each request.

The worker may still connect to an already-running ComfyUI process if the configured host and port are healthy before startup is needed. It must not clone repositories, download models, install dependencies, or mutate the prepared environment except for runtime outputs and temporary files.

Alternative considered: require the Provisioner Worker to leave ComfyUI running. That does not fit serverless lifecycle boundaries because the Provisioning Pod is temporary and is deleted before the persistent endpoint is used.

### Make Endpoint Worker deployment provider-specific now

Native build configuration will keep the Provisioner Worker image ref and port provider-neutral, while Endpoint Worker image refs and ports are configured per provider. This reflects current knowledge: the provisioner logic is portable across providers with mounted temporary compute, while endpoint invocation is provider-specific. V1 configures the RunPod Endpoint Worker only.

Alternative considered: keep one global Endpoint Worker image ref until a second provider exists. That would preserve the current env shape, but it would encode a provider-neutral assumption into the build contract even though endpoint invocation is already known to be provider-specific.

## Risks / Trade-offs

- Minimal prompt-only generation can become a dead-end if `t2i` needs more parameters -> Keep the contract explicitly v1 and add later requirements for typed parameters.
- Inline base64 images can exceed practical response sizes for larger outputs -> Limit v1 to exactly one image and defer durable artifact storage to a separate change.
- RunPod handler packaging may leak provider details into ComfyUI execution logic -> Keep provider request parsing separate from the ComfyUI adapter module.
- ComfyUI startup can dominate cold-start latency -> Accept this for v1 and leave warm-worker and startup optimization to later endpoint performance work.
- Readiness remains weaker than generation success -> Document that provisioning does not run generation and first generation can still fail as a runtime error.

## Migration Plan

This is a pre-production breaking configuration change. Add the new worker package and RunPod-specific endpoint worker build configuration, update `.env.example`, and replace the existing global endpoint worker env names with provider-qualified names.

Rollback is removing `workers/runpod-endpoint` and reverting the build configuration/doc updates before provisioning depends on the new image ref.

## Open Questions

- None.
