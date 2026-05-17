## MODIFIED Requirements

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
- **AND** it MUST NOT attempt to repair the prepared environment by creating a virtual environment, running pip, downloading assets, cloning repositories, or extracting runtime archives

#### Scenario: Prepared environment is missing
- **WHEN** the fixed image-baked ComfyUI runtime, fixed image-baked Python interpreter, required workflow definition, required model file, required Custom Node file, or declared overlay path is missing from the runtime environment
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** it MUST NOT attempt to repair the environment by downloading or installing missing assets

#### Scenario: ComfyUI generation fails
- **WHEN** ComfyUI rejects the workflow, fails during execution, times out, or does not produce an expected image output
- **THEN** the RunPod Endpoint Worker SHALL fail the request with a stable UI-safe error code
- **AND** the response MUST NOT include raw command output, filesystem secrets, provider API keys, or credential-bearing diagnostics

## REMOVED Requirements

### Requirement: Validate workspace-resolved runtime record paths
**Reason**: Runtime record path and image base dependency record validation are removed from the Endpoint Worker contract.
**Migration**: The Endpoint Worker validates only the prepared runtime manifest fields and workspace files needed to run generation with the fixed image-baked runtime.
