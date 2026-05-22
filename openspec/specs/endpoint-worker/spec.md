# Endpoint Worker Specification

## Purpose
Define the RunPod Endpoint Worker package, container, and temporary stubbed generation boundary.

## Requirements
### Requirement: Package RunPod Endpoint Worker runtime
The repository SHALL provide a RunPod-specific Endpoint Worker package and container image boundary with a temporary stubbed generation handler.

#### Scenario: Endpoint worker container starts
- **WHEN** the RunPod Endpoint Worker container starts in a RunPod Serverless worker environment
- **THEN** the RunPod Endpoint Worker SHALL initialize a RunPod-compatible serverless handler
- **AND** the RunPod Endpoint Worker SHALL wait for RunPod job invocations before returning the stubbed generation response
- **AND** it MUST NOT require a prepared runtime manifest during container startup

#### Scenario: Endpoint worker does not provision environment
- **WHEN** the RunPod Endpoint Worker handles startup or generation
- **THEN** it MUST NOT clone ComfyUI repositories, download model assets, install dependencies, install runtime extensions, create virtual environments, modify the image-baked runtime, or run pip
- **AND** it SHALL rely only on the image-baked worker package and stub configuration needed to respond to RunPod jobs
- **AND** it MUST NOT rely on provisioner-written Python path, ComfyUI root, image runtime root, model asset path list, output directory path, or prepared timestamp fields

### Requirement: Stub generation while preserving worker contract
The RunPod Endpoint Worker SHALL preserve the RunPod handler and response contract without executing ComfyUI generation in this change.

#### Scenario: Stubbed generation request is accepted
- **WHEN** a valid generation request is accepted
- **THEN** the RunPod Endpoint Worker SHALL return a deterministic UI-safe stub response that clearly represents generation as not implemented
- **AND** it MUST NOT start ComfyUI, contact a ComfyUI HTTP endpoint, submit a workflow, poll execution status, collect image outputs, or inspect model/output paths
- **AND** the response MUST NOT include raw command output, filesystem secrets, provider API keys, or credential-bearing details

#### Scenario: Runtime inputs are not prevalidated by stub
- **WHEN** the stubbed Endpoint Worker receives a request with workflow, model, output, or workspace path inputs
- **THEN** it MUST NOT treat those paths as authoritative prepared-runtime evidence
- **AND** it MUST NOT read a prepared runtime manifest, prevalidate model files, prevalidate output directories, or validate image-local ComfyUI paths

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
