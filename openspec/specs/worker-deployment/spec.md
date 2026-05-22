# worker-deployment Specification

## Purpose

Defines how LumaForge deploys worker container images from Git through repository automation.
## Requirements
### Requirement: Deploy Worker Images from Git
The repository SHALL provide worker image deployment workflows that deploy generic provisioner images and runtime-specific endpoint images from tracked Git source and the shared worker Dockerfile.

#### Scenario: Runtime recipe release deploys endpoint image
- **WHEN** a release tag or authorized workflow dispatch selects a runtime recipe
- **THEN** the runtime deployment workflow SHALL validate the Provisioner Worker and RunPod Endpoint Worker packages
- **AND** the runtime deployment workflow SHALL build the endpoint image compatible with the recipe-declared runtime contract id/version pair from the shared worker Dockerfile
- **AND** the endpoint image build SHALL install the selected recipe's image-baked base runtime, including ComfyUI revision, PyTorch index URL, and PyTorch package list
- **AND** the runtime deployment workflow MUST NOT forward recipe-owned base requirement files, runtime platform metadata, or ComfyUI repository URLs as Docker build inputs
- **AND** the runtime deployment workflow SHALL publish the endpoint image to GitHub Container Registry only after worker validation and endpoint image build validation succeed

#### Scenario: Manual dispatch deploys one runtime recipe
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** that workflow SHALL require a runtime recipe selection
- **AND** that workflow SHALL build, validate, publish, and catalog only the selected runtime recipe endpoint image
- **AND** that workflow SHALL publish an immutable image ref for the endpoint image

#### Scenario: Runtime catalog update is proposed
- **WHEN** a runtime deployment workflow publishes a validated endpoint image for a runtime recipe
- **THEN** it SHALL append a new bundled Runtime Catalog revision for the selected contract id using the next patch version, unless the recipe declares a higher SemVer version
- **AND** the new revision SHALL contain the published immutable Endpoint Worker image ref
- **AND** it SHALL update Workflow Presets using that runtime contract id to reference the new revision version
- **AND** it SHALL open a reviewed repository change for `bundled/runtime-catalog.json` and `bundled/workflow-catalog.json`
- **AND** it MUST NOT silently push runtime catalog changes directly to the main branch

### Requirement: Validate worker before publishing
Each runtime deployment workflow SHALL complete worker package validation and Docker image build validation before publishing runtime recipe endpoint images.

#### Scenario: Worker validation passes
- **WHEN** a workflow is preparing to publish a runtime recipe endpoint image
- **THEN** it SHALL run the test command for the Provisioner Worker package
- **AND** it SHALL run the test command for the RunPod Endpoint Worker package
- **AND** it SHALL run the Docker build for the endpoint worker image using the shared worker Dockerfile
- **AND** it SHALL continue to registry publication only after validation succeeds for the endpoint image

#### Scenario: Worker validation fails
- **WHEN** any required worker validation or image build validation step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag
- **AND** the workflow MUST NOT propose a Runtime Catalog update for the failed endpoint image

### Requirement: Tag published worker images deterministically

Each worker deployment workflow SHALL publish deterministic image tags that identify the source Git revision.

#### Scenario: Commit image tag is published
- **WHEN** a workflow publishes a worker image
- **THEN** it SHALL publish an immutable tag containing the source commit SHA
- **AND** the image tag SHALL identify the exact Git commit used for the build

#### Scenario: Release image tag is published
- **WHEN** a workflow is triggered by a release tag
- **THEN** it SHALL publish a version tag derived from that Git tag for the selected worker image
- **AND** it SHALL also publish an immutable commit SHA tag for the selected worker image

### Requirement: Protect registry credentials

The worker deployment workflow SHALL keep registry credentials and deployment secrets in GitHub Actions secrets or built-in GitHub tokens.

#### Scenario: Registry login uses GitHub token context
- **WHEN** the workflow authenticates to GitHub Container Registry
- **THEN** it SHALL read authentication from built-in GitHub token context
- **AND** committed workflow and documentation files MUST NOT contain plaintext registry credentials

#### Scenario: Deployment logs are emitted
- **WHEN** the workflow runs validation, build, and publish steps
- **THEN** logs MUST NOT print registry passwords, access tokens, provider API keys, or worker bearer tokens

### Requirement: Document worker deployment operation
The repository SHALL document how to operate the runtime recipe release workflow.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy worker images
- **THEN** documentation SHALL describe the runtime recipe workflow triggers
- **AND** documentation SHALL describe runtime recipe selection
- **AND** documentation SHALL describe automatic Runtime Catalog patch version selection
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced immutable image tags and digest-pinned refs
- **AND** documentation SHALL describe reviewed Runtime Catalog update PRs

### Requirement: Build deterministic ComfyUI runtime in endpoint image
The worker Docker build SHALL construct the deterministic ComfyUI base runtime for the selected runtime recipe inside endpoint images before they can be published.

#### Scenario: Image-baked runtime is built
- **WHEN** the endpoint worker image is built for a runtime recipe
- **THEN** the Docker build SHALL install the fixed Python runtime, recipe-declared PyTorch/CUDA-compatible dependencies, ComfyUI, ComfyUI frontend/docs/templates, and ComfyUI base requirements into the image runtime root
- **AND** ComfyUI base requirements SHALL be installed through the pinned `comfy-cli` ComfyUI installation path rather than through recipe-owned requirement file lists
- **AND** bundled workflow runtime extensions, if any, SHALL be part of the selected runtime image implementation rather than the workflow catalog or provisioning contract
- **AND** base runtime dependency installation MUST happen during Docker build rather than container startup or workspace provisioning

#### Scenario: Image-baked runtime build fails
- **WHEN** the Docker build cannot install or verify any deterministic ComfyUI runtime dependency
- **THEN** the Docker build SHALL fail
- **AND** no runtime recipe release workflow SHALL publish that endpoint image

### Requirement: Build generic Provisioner Worker image
The worker Docker build SHALL construct a generic Provisioner Worker image that contains only the worker application runtime, worker Python dependencies, and Hugging Face download tooling needed to prepare a mounted workspace.

#### Scenario: Provisioner image is built
- **WHEN** the Provisioner Worker image is built
- **THEN** the Docker build SHALL install the worker Python runtime, worker application dependencies, and Hugging Face download tooling required for model asset downloads
- **AND** the Provisioner Worker image MUST NOT install ComfyUI, PyTorch/CUDA runtime dependencies, ComfyUI base requirements, Workflow Preset runtime extensions, or runtime extension Python dependencies
- **AND** the Provisioner Worker image MUST NOT include a workflow-runtime-specific ComfyUI runtime contract
