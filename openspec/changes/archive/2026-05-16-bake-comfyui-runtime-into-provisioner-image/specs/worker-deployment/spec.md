## MODIFIED Requirements

### Requirement: Deploy Worker Images from Git
The repository SHALL provide a runtime recipe release workflow that deploys compatible worker container image pairs from tracked Git source, runtime recipe input, and the shared worker Dockerfile.

#### Scenario: Runtime recipe release deploys image pair
- **WHEN** a release tag or authorized workflow dispatch selects a runtime recipe
- **THEN** the runtime deployment workflow SHALL validate the Provisioner Worker and RunPod Endpoint Worker
- **AND** the runtime deployment workflow SHALL build the provisioner image for the recipe-declared runtime contract id/version and release-assigned implementation revision from the shared worker Dockerfile
- **AND** the runtime deployment workflow SHALL build the endpoint image compatible with the recipe-declared runtime contract id/version and release-assigned implementation revision from the shared worker Dockerfile
- **AND** the runtime deployment workflow SHALL publish both images to GitHub Container Registry only after pair validation succeeds

#### Scenario: Manual dispatch deploys one runtime recipe
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** that workflow SHALL require a runtime recipe selection
- **AND** that workflow SHALL build, validate, publish, and catalog only the selected runtime recipe image pair
- **AND** that workflow SHALL publish immutable image refs for both worker images

#### Scenario: Runtime catalog update is proposed
- **WHEN** a runtime deployment workflow publishes a validated image pair for a runtime recipe
- **THEN** it SHALL generate a new bundled Runtime Catalog entry or append a new implementation revision to an existing compatible entry from verified image metadata
- **AND** when appending an implementation revision for a non-rollback release, it SHALL preserve existing implementation revisions unchanged and advance the default implementation revision for future Workspaces
- **AND** it SHALL open a reviewed repository change for `bundled/runtime-catalog.json`
- **AND** it MUST NOT silently push runtime catalog changes directly to the main branch

### Requirement: Validate worker before publishing
Each runtime deployment workflow SHALL complete worker package validation, image build validation, and image-pair compatibility validation before publishing runtime recipe images.

#### Scenario: Worker validation passes
- **WHEN** a workflow is preparing to publish a runtime recipe image pair
- **THEN** it SHALL run the test command for the Provisioner Worker package
- **AND** it SHALL run the test command for the RunPod Endpoint Worker package
- **AND** it SHALL run Docker builds for both worker images using the shared worker Dockerfile
- **AND** it SHALL continue to registry publication only after validation succeeds for the image pair

#### Scenario: Endpoint compatibility validation passes
- **WHEN** the runtime deployment workflow has built both worker images
- **THEN** it SHALL verify the image pair declares the same runtime contract id, version, and implementation revision
- **AND** it SHALL continue only after pair compatibility validation succeeds

#### Scenario: Worker validation fails
- **WHEN** any required worker validation, image build validation, or image-pair compatibility step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag
- **AND** the workflow MUST NOT propose a Runtime Catalog update for the failed image pair

### Requirement: Document worker deployment operation
The repository SHALL document how to operate the runtime recipe release workflow.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy or roll back worker images
- **THEN** documentation SHALL describe the runtime recipe workflow triggers
- **AND** documentation SHALL describe runtime recipe selection
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced immutable image tags and digest-pinned refs
- **AND** documentation SHALL describe implementation revision increments for worker-only redeploys
- **AND** documentation SHALL describe reviewed Runtime Catalog update PRs
- **AND** documentation SHALL describe rollback by selecting previously published immutable image pairs from Runtime Catalog entries

## ADDED Requirements

### Requirement: Build deterministic ComfyUI runtime in provisioner image
The provisioner worker Docker build SHALL construct the deterministic ComfyUI base runtime archive for the selected runtime recipe before the image can be published.

#### Scenario: Runtime archive is built
- **WHEN** the provisioner worker image is built for a runtime recipe
- **THEN** the Docker build SHALL install the fixed Python runtime, PyTorch/CUDA-compatible dependencies, ComfyUI, ComfyUI frontend/docs/templates, and ComfyUI base requirements into the runtime archive
- **AND** the Docker build SHALL produce metadata describing the runtime contract, implementation revision, and included base runtime revisions
- **AND** the Docker build MUST NOT install Workflow Preset Custom Nodes or their Python dependencies into the runtime archive
- **AND** base runtime dependency installation MUST happen during Docker build rather than container startup or workspace provisioning

#### Scenario: Runtime archive build fails
- **WHEN** the Docker build cannot install or verify any deterministic ComfyUI runtime dependency
- **THEN** the Docker build SHALL fail
- **AND** no runtime recipe release workflow SHALL publish that image pair
