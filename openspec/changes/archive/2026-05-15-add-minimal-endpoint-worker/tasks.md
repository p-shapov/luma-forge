## 1. Native Build Configuration

- [x] 1.1 Rename endpoint worker build env keys to RunPod-qualified names while keeping provisioner worker env keys provider-neutral.
- [x] 1.2 Update native build configuration parsing and Cargo env emission for RunPod Endpoint Worker image ref and port.
- [x] 1.3 Update `.env.example`, development docs, and relevant flow/glossary docs to describe provider-neutral provisioner config and RunPod-specific endpoint config.
- [x] 1.4 Verify renamed native build configuration through existing native backend verification.

## 2. RunPod Endpoint Worker Package

- [x] 2.1 Create `workers/runpod-endpoint` package metadata, source layout, test layout, and README.
- [x] 2.2 Add RunPod Endpoint Worker configuration for prepared workspace mount path, ComfyUI host/port, request limits, generation timeout, and supported execution types.
- [x] 2.3 Add request and response schemas for minimal execution-type plus text prompt input, single-image output, success status, and stable UI-safe error responses.
- [x] 2.4 Add secret-safe logging and error mapping utilities for invalid input, missing prepared environment, ComfyUI startup failure, ComfyUI execution failure, timeout, and unexpected runtime failure.

## 3. RunPod Handler Boundary

- [x] 3.1 Add a RunPod Serverless handler entrypoint that accepts provider job input and delegates to provider-neutral generation service code.
- [x] 3.2 Validate execution type and prompt shape before generation, and reject unsupported execution types plus missing, blank, non-string, and oversized prompts without calling ComfyUI.
- [x] 3.3 Ensure handler responses do not include provider API keys, worker tokens, environment dumps, raw stack traces, or generated image data in logs.

## 4. ComfyUI Runtime Adapter

- [x] 4.1 Implement prepared-environment validation for ComfyUI root, `t2i` workflow definition, required model paths, and required Custom Node paths.
- [x] 4.2 Implement ComfyUI process startup or connection handling against the prepared workspace without cloning, downloading, or installing assets.
- [x] 4.3 Implement prompt substitution into the known workflow JSON for the supported preset.
- [x] 4.4 Submit the workflow to ComfyUI, wait for completion within the configured timeout, and collect expected image outputs.
- [x] 4.5 Return exactly one generated image as MIME type plus base64 image data in the minimal success response.

## 5. Container Boundary

- [x] 5.1 Add a RunPod Endpoint Worker Dockerfile that packages the RunPod handler and required runtime dependencies.
- [x] 5.2 Ensure the container starts the Endpoint Worker handler by default and does not run provisioning or generation work on boot.
- [x] 5.3 Add container smoke verification that the image imports and initializes the handler without starting a generation request.

## 6. Tests

- [x] 6.1 Add unit tests for valid and invalid generation request parsing.
- [x] 6.2 Add unit tests for UI-safe error mapping and secret redaction boundaries.
- [x] 6.3 Add unit tests for prepared-environment validation success and failure cases.
- [x] 6.4 Add unit tests for ComfyUI adapter success, timeout, missing output, and execution failure using mocked ComfyUI calls.
- [x] 6.5 Add handler tests proving invalid input does not call ComfyUI and successful generation returns image metadata plus base64 data.

## 7. Verification

- [x] 7.1 Run the RunPod Endpoint Worker test suite.
- [x] 7.2 Build the RunPod Endpoint Worker container image.
- [x] 7.3 Run native backend verification required for `src-tauri/` changes.
- [x] 7.4 Run `openspec validate add-minimal-endpoint-worker` and confirm the change is apply-ready.
