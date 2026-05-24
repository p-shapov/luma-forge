# Endpoint Worker Specification

## Purpose
Define the RunPod Endpoint Worker package, container, and workflow-specific ComfyUI generation boundary.

## Requirements

### Requirement: Package RunPod Endpoint Worker runtime
The repository SHALL provide a RunPod-specific Endpoint Worker package and container image boundary that initializes a RunPod-compatible handler and executes the selected image-baked runtime at request time.

#### Scenario: Endpoint worker container starts
- **WHEN** the RunPod Endpoint Worker container starts in a RunPod Serverless worker environment
- **THEN** the RunPod Endpoint Worker SHALL initialize a RunPod-compatible serverless handler
- **AND** the RunPod Endpoint Worker SHALL wait for RunPod job invocations before executing generation
- **AND** it MUST NOT require a prepared runtime manifest during container startup

#### Scenario: Endpoint worker does not provision environment
- **WHEN** the RunPod Endpoint Worker handles startup or generation
- **THEN** it MUST NOT clone ComfyUI repositories, download model assets, install dependencies, install runtime extensions, create virtual environments, modify the image-baked runtime, or run pip
- **AND** it SHALL rely on the image-baked worker package, ComfyUI runtime, Comfy CLI installation, baked workflow file, and mounted workspace model assets
- **AND** it MUST NOT rely on provisioner-written Python path, ComfyUI root, image runtime root, model asset path list, output directory path, or prepared timestamp fields

### Requirement: Execute bundled HiDream workflow through Comfy CLI
The RunPod Endpoint Worker SHALL execute the bundled HiDream O1 Dev ComfyUI UI workflow through Comfy CLI when it receives a valid text-to-image generation request.

#### Scenario: Valid generation request executes ComfyUI
- **WHEN** the Endpoint Worker receives a valid `t2i` request with a non-empty prompt
- **THEN** it SHALL start ComfyUI if the worker process does not already have a ready local ComfyUI server
- **AND** it SHALL create a temporary copy of the baked UI workflow
- **AND** it SHALL patch the HiDream `User Prompt` node id `171` with the request prompt
- **AND** it SHALL patch the HiDream `Switch to Image Edit` node id `154` to `false`
- **AND** it SHALL patch the HiDream `Enable Prompt Refine?` node id `177` to `false`
- **AND** it SHALL run the patched UI workflow with Comfy CLI against the local ComfyUI server
- **AND** it SHALL return a succeeded response with `generation.implemented` set to `true`

#### Scenario: Baked workflow shape changes unexpectedly
- **WHEN** the baked workflow does not contain the expected HiDream node ids, node types, or node titles needed by the smoke execution path
- **THEN** the Endpoint Worker SHALL fail the request safely
- **AND** it MUST NOT submit a partially patched or unknown workflow to ComfyUI

#### Scenario: Generated output is returned
- **WHEN** ComfyUI completes the workflow with image output metadata
- **THEN** the Endpoint Worker SHALL fetch the generated image through the local ComfyUI output URL
- **AND** it SHALL return base64 image data and UI-safe image metadata in the generation response
- **AND** it MUST NOT return raw command output, stack traces, provider API keys, worker bearer tokens, or credential-bearing filesystem details

### Requirement: Report endpoint generation failures with diagnostic error metadata
The RunPod Endpoint Worker SHALL return structured UI-safe diagnostic error metadata for worker-handled failed generation requests.

#### Scenario: Failed endpoint response includes stable error metadata
- **WHEN** the Endpoint Worker handles a generation request and fails before returning a successful generation response
- **THEN** the failed response SHALL include `status` set to `failed`
- **AND** the failed response SHALL include `error.code` with a stable endpoint worker error classifier
- **AND** the failed response SHALL include `error.message` with a UI-safe diagnostic message
- **AND** the failed response MUST NOT include raw command output, stack traces, provider API keys, worker bearer tokens, authorization headers, environment dumps, credential-bearing filesystem details, or generated image data

#### Scenario: ComfyUI subprocess failure message uses non-raw metadata
- **WHEN** ComfyUI startup or workflow execution fails with subprocess failure metadata available to the worker
- **THEN** the failed response `error.message` SHALL include a bounded diagnostic message that identifies the failed stage
- **AND** the failed response `error.code` SHALL identify the ComfyUI failure stage
- **AND** the failed response MAY include non-raw process metadata such as exit status or timeout duration
- **AND** the failed response MUST NOT expose secrets, raw stdout, raw stderr, raw command output, stack traces, environment dumps, command invocations, or generated image data

#### Scenario: Endpoint worker maps known failure stages to stable codes
- **WHEN** request validation, workflow validation, ComfyUI launch, ComfyUI startup timeout, workflow execution, workflow timeout, output parsing, missing outputs, output fetching, response-size validation, or unexpected runtime handling fails
- **THEN** the failed response SHALL use the most specific stable endpoint worker error code available for that failure stage

### Requirement: Configure endpoint workspace mount path from Native provisioning
The RunPod Endpoint Worker SHALL support the Native-provided workspace mount path when its template is created.

#### Scenario: Endpoint Worker receives shared workspace mount path
- **WHEN** the Endpoint Worker container starts with `LUMA_FORGE_WORKSPACE_MOUNT_PATH` set to an absolute safe path
- **THEN** the Endpoint Worker SHALL use that path as its shared prepared workspace root unless an endpoint-specific override is configured
- **AND** it MUST NOT assume `/workspace` when a valid Native-provided mount path is present

#### Scenario: Endpoint Worker template is mounted at the configured path
- **WHEN** the Native Layer creates an Endpoint Worker template with a resolved workspace volume mount path
- **THEN** the Endpoint Worker container SHALL receive the same value through `LUMA_FORGE_WORKSPACE_MOUNT_PATH`
- **AND** the RunPod template volume mount path SHALL match the worker environment value

### Requirement: Include selected workflow in workflow-specific endpoint image
The Endpoint Worker image built for a workflow-specific runtime contract SHALL include the selected bundled UI workflow at a fixed image-local path owned by the endpoint runtime implementation.

#### Scenario: Workflow-specific endpoint image is built
- **WHEN** the Endpoint Worker image is built for runtime contract id `comfyui-hidream-o1-dev-python312-cu121`
- **THEN** the image SHALL contain the workflow derived from `bundled/workflows/comfyui-hidream-o1-dev.json`
- **AND** the workflow SHALL be copied to a fixed image-local path selected by the endpoint runtime implementation
- **AND** the image build validation SHALL prove the fixed workflow file exists
