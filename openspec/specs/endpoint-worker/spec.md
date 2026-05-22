# endpoint-worker Specification

## Purpose

Define the runtime worker contract used behind provider-managed Serverless Endpoints for generation against a prepared ComfyUI environment.
## Requirements
### Requirement: Package RunPod Endpoint Worker runtime
The repository SHALL provide a RunPod-specific Endpoint Worker package and container image boundary for running generation behind a RunPod Serverless endpoint.

#### Scenario: Endpoint worker container starts
- **WHEN** the RunPod Endpoint Worker container starts in a RunPod Serverless worker environment
- **THEN** the RunPod Endpoint Worker SHALL initialize a RunPod-compatible serverless handler
- **AND** the RunPod Endpoint Worker SHALL validate its image runtime configuration before accepting generation work
- **AND** the RunPod Endpoint Worker SHALL wait for RunPod job invocations before running generation

#### Scenario: Endpoint worker does not provision environment
- **WHEN** the RunPod Endpoint Worker handles startup or generation
- **THEN** it MUST NOT clone ComfyUI repositories, download model assets, install dependencies, install Custom Nodes, create virtual environments, modify the image-baked runtime, or run pip
- **AND** it SHALL rely on the image-baked base runtime plus the prepared workspace manifest, workspace Custom Nodes, workspace models, workspace output paths, and workspace Python overlay

### Requirement: Accept minimal execution-type generation input

The RunPod Endpoint Worker SHALL accept a minimal generation request containing an execution type and text prompt. The RunPod v1 Endpoint Worker SHALL support the `t2i` execution type.

#### Scenario: Valid generation input is accepted

- **WHEN** RunPod invokes the Endpoint Worker with an input object containing execution type `t2i` and a non-empty text prompt
- **THEN** the RunPod Endpoint Worker SHALL accept the request for generation
- **AND** it SHALL treat the prompt as the user text input for the `t2i` workflow

#### Scenario: Invalid generation input is rejected

- **WHEN** RunPod invokes the Endpoint Worker with a missing, blank, non-string, or oversized prompt
- **THEN** the RunPod Endpoint Worker SHALL reject the request with a stable UI-safe error code
- **AND** it MUST NOT submit a prompt to ComfyUI for that request

#### Scenario: Unsupported execution type is rejected

- **WHEN** RunPod invokes the Endpoint Worker with a missing, blank, non-string, or unsupported execution type
- **THEN** the RunPod Endpoint Worker SHALL reject the request with a stable UI-safe error code
- **AND** it MUST NOT submit a prompt to ComfyUI for that request

### Requirement: Execute generation through prepared ComfyUI
The RunPod Endpoint Worker SHALL execute accepted generation requests by using the fixed image-baked ComfyUI runtime and the prepared workspace mounted in the endpoint worker environment.

#### Scenario: ComfyUI starts lazily before generation
- **WHEN** a valid generation request is accepted
- **AND** the configured ComfyUI HTTP endpoint is not already ready
- **THEN** the RunPod Endpoint Worker SHALL validate the prepared runtime manifest before starting ComfyUI
- **AND** it SHALL start the ComfyUI process from the fixed image-baked ComfyUI root using the fixed image-baked Python interpreter
- **AND** it SHALL configure workspace model paths, Custom Node paths, output paths, and Python overlay paths from the runtime manifest
- **AND** it SHALL wait for `/system_stats` before submitting the workflow
- **AND** it MUST NOT start a separate ComfyUI process per request

#### Scenario: Existing ComfyUI process is reused
- **WHEN** a valid generation request is accepted
- **AND** the configured ComfyUI HTTP endpoint is already ready
- **THEN** the RunPod Endpoint Worker SHALL reuse that process
- **AND** it MUST NOT start another ComfyUI process

#### Scenario: ComfyUI generation succeeds
- **WHEN** a valid generation request is accepted
- **AND** the prepared ComfyUI runtime can be started or reached
- **AND** ComfyUI completes the known `t2i` workflow execution
- **THEN** the RunPod Endpoint Worker SHALL collect the generated image output
- **AND** it SHALL return a successful generation response

#### Scenario: Prepared runtime manifest is invalid
- **WHEN** a valid generation request is accepted
- **AND** the prepared runtime manifest is missing, invalid, or does not declare required workspace-specific prepared paths
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared environment by creating a virtual environment, running pip, downloading assets, cloning repositories, or modifying the image-baked runtime

#### Scenario: Prepared environment is missing
- **WHEN** the fixed image-baked ComfyUI runtime, fixed image-baked Python interpreter, required workflow definition, required model file, required Custom Node file, or declared overlay path is missing from the runtime environment
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** it MUST NOT attempt to repair the environment by downloading or installing missing assets

#### Scenario: ComfyUI generation fails
- **WHEN** ComfyUI rejects the workflow, fails during execution, times out, or does not produce an expected image output
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** the response MUST NOT include raw command output, filesystem secrets, provider API keys, or credential-bearing details

### Requirement: Return minimal image output

The RunPod Endpoint Worker SHALL return generated image output using a RunPod-job-safe JSON response shape.

#### Scenario: Image output is returned

- **WHEN** generation succeeds
- **THEN** the RunPod Endpoint Worker SHALL return exactly one generated image with MIME type and base64-encoded image data
- **AND** the response SHALL include a terminal success status

#### Scenario: No persistent artifact contract is exposed

- **WHEN** generation succeeds
- **THEN** the Endpoint Worker MUST NOT require object storage, public URLs, or app gallery persistence to return the minimal result

### Requirement: Keep Endpoint Worker responses secret-safe

The RunPod Endpoint Worker SHALL keep all provider responses, error metadata, and logs free of secrets and unsafe internal details.

#### Scenario: Runtime error is reported safely

- **WHEN** the RunPod Endpoint Worker reports an invalid request, missing environment, ComfyUI startup failure, ComfyUI execution failure, timeout, or unexpected runtime failure
- **THEN** the response SHALL include a stable UI-safe error code and optional UI-safe message
- **AND** the response MUST NOT include provider API keys, worker bearer tokens, environment dumps, raw stack traces, or credential-bearing command output

#### Scenario: Logs are written

- **WHEN** the RunPod Endpoint Worker writes startup, request, or error logs
- **THEN** logs MUST NOT include provider API keys, worker bearer tokens, full request secrets, or generated image data

### Requirement: Exclude generation from provisioning readiness

Workspace Provisioning SHALL NOT invoke the Endpoint Worker generation contract as part of provisioning readiness validation.

#### Scenario: Workspace provisioning validates endpoint readiness

- **WHEN** Workspace Provisioning validates the persistent runtime entry point
- **THEN** it SHALL validate provider resource metadata and no-job endpoint health or status information only
- **AND** it MUST NOT submit a generation request to the Endpoint Worker

#### Scenario: First generation fails after provisioning

- **WHEN** a Workspace is marked `Ready`
- **AND** the user's first generation request fails in the Endpoint Worker
- **THEN** the failure SHALL be treated as a generation/runtime failure
- **AND** it MUST NOT retroactively mean that Workspace Provisioning submitted or should have submitted a hidden generation job
