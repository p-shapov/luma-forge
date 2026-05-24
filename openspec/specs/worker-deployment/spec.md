# Worker Deployment Specification

## Purpose
Define separate worker image deployment paths for generic provisioner images and runtime contract endpoint images.
## Requirements
### Requirement: Deploy Worker Images from Git
The repository SHALL provide separate worker image deployment workflows that deploy generic provisioner images and runtime-specific endpoint images from tracked Git source, then automatically propose catalog promotion for successfully published digest-pinned worker images.

#### Scenario: Provisioner release deploys generic image
- **WHEN** a release tag or authorized workflow dispatch selects the Provisioner Worker release path
- **THEN** the provisioner deployment workflow SHALL validate the Provisioner Worker package
- **AND** it SHALL build the generic Provisioner Worker image without runtime contract build arguments
- **AND** it SHALL publish the provisioner image to GitHub Container Registry only after validation and image build validation succeed
- **AND** it MUST NOT update Runtime Catalog revisions

#### Scenario: Runtime contract release deploys endpoint image
- **WHEN** a release tag or authorized workflow dispatch selects a runtime contract
- **THEN** the runtime deployment workflow SHALL validate the RunPod Endpoint Worker package and runtime contract tooling
- **AND** the runtime deployment workflow SHALL build the endpoint image compatible with the selected runtime contract id/version
- **AND** the endpoint image build SHALL install the selected contract's required endpoint Python and system dependencies
- **AND** the endpoint image SHALL include the bundled workflow derived from the selected contract's `contract.id`
- **AND** the endpoint image SHALL install contract-declared image-baked runtime dependencies such as ComfyUI revision, PyTorch index URL, PyTorch package list, Comfy CLI, and a bundled workflow file for request-time ComfyUI execution
- **AND** the runtime deployment workflow MUST NOT forward contract-owned base requirement files, runtime platform metadata, or ComfyUI repository URLs as Docker build inputs
- **AND** the runtime deployment workflow SHALL publish the endpoint image to GitHub Container Registry only after worker validation and endpoint image build validation succeed

#### Scenario: Manual dispatch deploys one runtime contract
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** that workflow SHALL require a runtime contract selection
- **AND** that workflow SHALL build, validate, publish, and promote only the selected runtime contract endpoint image
- **AND** that workflow SHALL publish an immutable image ref for the endpoint image

#### Scenario: Manual dispatch deploys provisioner contract
- **WHEN** an authorized operator starts the provisioner deployment workflow manually
- **THEN** that workflow SHALL build, validate, publish, and promote the generic Provisioner Worker image for the bundled provisioner contract
- **AND** that workflow SHALL publish an immutable image ref for the Provisioner Worker image

#### Scenario: Runtime image promotion is proposed
- **WHEN** a runtime deployment workflow publishes a validated endpoint image for a runtime contract
- **THEN** it SHALL promote the published image into the bundled Runtime Catalog by appending a new revision for the selected contract id using the next patch version from the current Runtime Catalog, unless the selected runtime contract declares a higher SemVer version
- **AND** the new revision SHALL contain the published immutable Endpoint Worker image ref
- **AND** it SHALL update the Workflow Preset whose id matches the runtime contract id to reference the new revision version
- **AND** it SHALL open a reviewed repository change for `bundled/runtime-catalog.json` and `bundled/workflow-catalog.json`
- **AND** it MUST NOT silently push runtime catalog changes directly to the main branch

#### Scenario: Provisioner image promotion is proposed
- **WHEN** a provisioner deployment workflow publishes a validated Provisioner Worker image
- **THEN** it SHALL promote the published image into the bundled Provisioner Catalog by appending a new revision for the provisioner contract id using the next patch version from the current Provisioner Catalog
- **AND** the new revision SHALL contain the published immutable Provisioner Worker image ref
- **AND** the new revision SHALL preserve the provisioner revision metadata needed by Workspace Setup
- **AND** it SHALL update Workflow Presets using that provisioner contract id to reference the new revision version
- **AND** it SHALL open a reviewed repository change for `bundled/provisioner-catalog.json` and `bundled/workflow-catalog.json`
- **AND** it MUST NOT silently push provisioner catalog changes directly to the main branch

### Requirement: Validate worker before publishing
Each worker deployment workflow SHALL complete package validation and Docker image build validation before publishing its image.

#### Scenario: Provisioner validation passes
- **WHEN** a workflow is preparing to publish the generic Provisioner Worker image
- **THEN** it SHALL run the test command for the Provisioner Worker package
- **AND** it SHALL run the Docker build for the provisioner image using the provisioner build definition
- **AND** it SHALL continue to registry publication only after validation succeeds for the provisioner image

#### Scenario: Endpoint validation passes
- **WHEN** a workflow is preparing to publish a runtime contract endpoint image
- **THEN** it SHALL run the test command for the RunPod Endpoint Worker package
- **AND** it SHALL run runtime contract tests
- **AND** it SHALL run the Docker build for the endpoint image using the endpoint build definition
- **AND** the endpoint image build validation SHALL prove required endpoint dependencies are installed without requiring real ComfyUI execution
- **AND** the endpoint image build validation SHALL prove the final endpoint image can resolve Comfy CLI as plain `comfy` from the runtime process environment
- **AND** it SHALL continue to registry publication only after validation succeeds for the endpoint image

#### Scenario: Worker validation fails
- **WHEN** any required worker validation or image build validation step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag
- **AND** endpoint runtime workflows MUST NOT propose Runtime Catalog promotion for the failed endpoint image
- **AND** provisioner workflows MUST NOT propose Provisioner Catalog promotion for the failed provisioner image

### Requirement: Document worker deployment operation
The repository SHALL document how to operate the provisioner image release workflow and the runtime contract endpoint image release workflow.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy worker images
- **THEN** documentation SHALL describe the separate provisioner and endpoint runtime workflow triggers
- **AND** documentation SHALL describe runtime contract selection for endpoint images
- **AND** documentation SHALL describe automatic Runtime Catalog patch version selection for endpoint image promotion
- **AND** documentation SHALL describe automatic Provisioner Catalog patch version selection for provisioner image promotion
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced immutable image tags and digest-pinned refs
- **AND** documentation SHALL describe reviewed Runtime Catalog promotion PRs for endpoint runtime releases
- **AND** documentation SHALL describe reviewed Provisioner Catalog promotion PRs for provisioner releases
- **AND** documentation SHALL describe that operators must not publish another worker image for the same catalog contract while a catalog promotion PR for that contract remains open

### Requirement: Build generic Provisioner Worker image
The worker Docker build SHALL construct a generic Provisioner Worker image that contains only the worker application runtime, worker Python dependencies, and Hugging Face download tooling needed to prepare a mounted workspace.

#### Scenario: Provisioner image is built
- **WHEN** the Provisioner Worker image is built
- **THEN** the Docker build SHALL install the worker Python runtime, worker application dependencies, and Hugging Face download tooling required for model asset downloads
- **AND** the Provisioner Worker image MUST NOT install ComfyUI, PyTorch/CUDA runtime dependencies, ComfyUI base requirements, Workflow Preset runtime extensions, runtime extension Python dependencies, or endpoint workflow files
- **AND** the Provisioner Worker image MUST NOT include or depend on a workflow-runtime-specific ComfyUI runtime contract
- **AND** the Provisioner Worker build MUST NOT depend on endpoint runtime Docker stages or runtime contract build arguments

### Requirement: Build Endpoint Worker image
The worker Docker build SHALL construct an Endpoint Worker image that contains the worker application runtime, selected bundled workflow, Comfy CLI, and contract-declared dependencies needed for request-time ComfyUI execution.

#### Scenario: Endpoint image is built
- **WHEN** the Endpoint Worker image is built for a selected runtime contract
- **THEN** the Docker build SHALL install the worker Python runtime, worker application dependencies, and contract-declared endpoint runtime dependencies
- **AND** the Docker build SHALL install `comfy-cli==1.10.3`
- **AND** the Docker build SHALL expose the image-baked runtime virtual environment executable directory on `PATH` in the final endpoint image
- **AND** the Docker build SHALL copy the bundled workflow derived from the selected contract's `contract.id` into a fixed image-local workflow path
- **AND** the image build validation SHALL prove the fixed image-local workflow file exists
- **AND** the image build validation SHALL prove ComfyUI and Comfy CLI are present in the image
- **AND** the image build validation SHALL prove plain `comfy` resolves to the image-baked runtime Comfy CLI executable in the final endpoint image
- **AND** the image build validation SHALL NOT require live GPU workflow execution or image output collection
- **AND** the built image SHALL preserve the RunPod-compatible handler entrypoint used by the Endpoint Worker
