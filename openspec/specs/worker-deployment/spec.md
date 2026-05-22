# Worker Deployment Specification

## Purpose
Define separate worker image deployment paths for generic provisioner images and runtime contract endpoint images.

## Requirements
### Requirement: Deploy Worker Images from Git
The repository SHALL provide separate worker image deployment workflows that deploy generic provisioner images and runtime-specific endpoint images from tracked Git source.

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
- **AND** the endpoint image MAY install contract-declared image-baked runtime dependencies such as ComfyUI revision, PyTorch index URL, and PyTorch package list even while request handling remains stubbed
- **AND** the runtime deployment workflow MUST NOT forward contract-owned base requirement files, runtime platform metadata, or ComfyUI repository URLs as Docker build inputs
- **AND** the runtime deployment workflow SHALL publish the endpoint image to GitHub Container Registry only after worker validation and endpoint image build validation succeed

#### Scenario: Manual dispatch deploys one runtime contract
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** that workflow SHALL require a runtime contract selection
- **AND** that workflow SHALL build, validate, publish, and catalog only the selected runtime contract endpoint image
- **AND** that workflow SHALL publish an immutable image ref for the endpoint image

#### Scenario: Runtime catalog update is proposed
- **WHEN** a runtime deployment workflow publishes a validated endpoint image for a runtime contract
- **THEN** it SHALL append a new bundled Runtime Catalog revision for the selected contract id using the next patch version, unless the selected runtime contract declares a higher SemVer version
- **AND** the new revision SHALL contain the published immutable Endpoint Worker image ref
- **AND** it SHALL update Workflow Presets using that runtime contract id to reference the new revision version
- **AND** it SHALL open a reviewed repository change for `bundled/runtime-catalog.json` and `bundled/workflow-catalog.json`
- **AND** it MUST NOT silently push runtime catalog changes directly to the main branch

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
- **AND** it SHALL continue to registry publication only after validation succeeds for the endpoint image

#### Scenario: Worker validation fails
- **WHEN** any required worker validation or image build validation step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag
- **AND** endpoint runtime workflows MUST NOT propose a Runtime Catalog update for the failed endpoint image

### Requirement: Document worker deployment operation
The repository SHALL document how to operate the provisioner image release workflow and the runtime contract endpoint image release workflow.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy worker images
- **THEN** documentation SHALL describe the separate provisioner and endpoint runtime workflow triggers
- **AND** documentation SHALL describe runtime contract selection for endpoint images
- **AND** documentation SHALL describe automatic Runtime Catalog patch version selection for endpoint images
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced immutable image tags and digest-pinned refs
- **AND** documentation SHALL describe reviewed Runtime Catalog update PRs for endpoint runtime releases

### Requirement: Build generic Provisioner Worker image
The worker Docker build SHALL construct a generic Provisioner Worker image that contains only the worker application runtime, worker Python dependencies, and Hugging Face download tooling needed to prepare a mounted workspace.

#### Scenario: Provisioner image is built
- **WHEN** the Provisioner Worker image is built
- **THEN** the Docker build SHALL install the worker Python runtime, worker application dependencies, and Hugging Face download tooling required for model asset downloads
- **AND** the Provisioner Worker image MUST NOT install ComfyUI, PyTorch/CUDA runtime dependencies, ComfyUI base requirements, Workflow Preset runtime extensions, runtime extension Python dependencies, or endpoint workflow files
- **AND** the Provisioner Worker image MUST NOT include or depend on a workflow-runtime-specific ComfyUI runtime contract
- **AND** the Provisioner Worker build MUST NOT depend on endpoint runtime Docker stages or runtime contract build arguments

### Requirement: Build stubbed Endpoint Worker image
The worker Docker build SHALL construct an Endpoint Worker image that contains the worker application runtime and contract-declared dependencies while request handling remains stubbed.

#### Scenario: Endpoint image is built
- **WHEN** the Endpoint Worker image is built for a selected runtime contract
- **THEN** the Docker build SHALL install the worker Python runtime, worker application dependencies, and contract-declared endpoint runtime dependencies
- **AND** the image build validation SHALL NOT require live ComfyUI startup, workflow submission, model resolution, or image output collection
- **AND** the built image SHALL preserve the RunPod-compatible handler entrypoint used by the stubbed Endpoint Worker
