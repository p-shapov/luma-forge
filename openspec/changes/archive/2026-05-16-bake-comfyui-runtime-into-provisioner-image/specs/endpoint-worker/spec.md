## MODIFIED Requirements

### Requirement: Package RunPod Endpoint Worker runtime
The repository SHALL provide a RunPod-specific Endpoint Worker package and container image boundary for running generation behind a RunPod Serverless endpoint.

#### Scenario: Endpoint worker container starts
- **WHEN** the RunPod Endpoint Worker container starts in a RunPod Serverless worker environment
- **THEN** the RunPod Endpoint Worker SHALL initialize a RunPod-compatible serverless handler
- **AND** the RunPod Endpoint Worker SHALL wait for RunPod job invocations before running generation

#### Scenario: Endpoint worker does not provision environment
- **WHEN** the RunPod Endpoint Worker handles startup or generation
- **THEN** it MUST NOT clone ComfyUI repositories, download model assets, install dependencies, install Custom Nodes, create virtual environments, or run pip
- **AND** it SHALL rely on the prepared ComfyUI environment and materialized volume-local virtual environment mounted into the runtime container

### Requirement: Execute generation through prepared ComfyUI
The RunPod Endpoint Worker SHALL execute accepted generation requests by using the prepared ComfyUI runtime and materialized volume-local Python environment mounted in the endpoint worker environment.

#### Scenario: ComfyUI starts lazily before generation
- **WHEN** a valid generation request is accepted
- **AND** the configured ComfyUI HTTP endpoint is not already ready
- **THEN** the RunPod Endpoint Worker SHALL validate the prepared runtime manifest before starting ComfyUI
- **AND** it SHALL start the prepared ComfyUI process from the mounted ComfyUI root using the materialized volume-local Python interpreter declared by the runtime manifest
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
- **AND** the prepared runtime manifest is missing, invalid, or does not declare the Workspace's resolved runtime contract implementation and materialized image-baked runtime
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe prepared runtime error
- **AND** it MUST NOT attempt to repair the prepared environment by creating a virtual environment, running pip, downloading assets, or cloning repositories

#### Scenario: Prepared environment is missing
- **WHEN** the prepared ComfyUI runtime, materialized Python interpreter, required workflow definition, required model file, or required Custom Node file is missing from the mounted environment
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** it MUST NOT attempt to repair the prepared environment by downloading or installing missing assets

#### Scenario: ComfyUI generation fails
- **WHEN** ComfyUI rejects the workflow, fails during execution, times out, or does not produce an expected image output
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** the response MUST NOT include raw command output, filesystem secrets, provider API keys, or credential-bearing diagnostics
