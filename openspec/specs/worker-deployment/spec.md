# worker-deployment Specification

## Purpose

Defines how LumaForge deploys worker container images from Git through repository automation.
## Requirements
### Requirement: Deploy Worker Images from Git
The repository SHALL provide a runtime recipe release workflow that deploys compatible worker container image pairs from tracked Git source, runtime recipe input, and the shared worker Dockerfile.

#### Scenario: Runtime recipe release deploys image pair
- **WHEN** a release tag or authorized workflow dispatch selects a runtime recipe
- **THEN** the runtime deployment workflow SHALL validate the Provisioner Worker and RunPod Endpoint Worker
- **AND** the runtime deployment workflow SHALL build the provisioner image for the recipe-declared runtime contract id/version and release-assigned implementation revision from the shared worker Dockerfile
- **AND** the runtime deployment workflow SHALL build the endpoint image compatible with the recipe-declared runtime contract id/version and release-assigned implementation revision from the shared worker Dockerfile
- **AND** both image builds SHALL install the selected recipe's image-baked base runtime, including ComfyUI repository, ComfyUI revision, PyTorch index URL, PyTorch package list, base requirements, dependency records, and runtime identity metadata
- **AND** the runtime deployment workflow SHALL publish both images to GitHub Container Registry only after pair validation and runtime contract compatibility validation succeed

#### Scenario: Manual dispatch deploys one runtime recipe
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** that workflow SHALL require a runtime recipe selection
- **AND** that workflow SHALL build, validate, publish, and catalog only the selected runtime recipe image pair
- **AND** that workflow SHALL publish immutable image refs for both worker images

#### Scenario: Runtime catalog update is proposed
- **WHEN** a runtime deployment workflow publishes a validated image pair for a runtime recipe
- **THEN** it SHALL generate a new bundled Runtime Catalog entry or append a new implementation revision to an existing compatible entry from verified image metadata, image runtime metadata, base dependency records, runtime manifest compatibility metadata, and overlay policy metadata
- **AND** when appending an implementation revision for a non-rollback release, it SHALL preserve existing implementation revisions unchanged and advance the default implementation revision for future Workspaces
- **AND** it SHALL open a reviewed repository change for `bundled/runtime-catalog.json`
- **AND** it MUST NOT silently push runtime catalog changes directly to the main branch

### Requirement: Validate worker before publishing
Each runtime deployment workflow SHALL complete worker package validation, image build validation, image-pair compatibility validation, image runtime validation, and runtime contract compatibility validation before publishing runtime recipe images.

#### Scenario: Worker validation passes
- **WHEN** a workflow is preparing to publish a runtime recipe image pair
- **THEN** it SHALL run the test command for the Provisioner Worker package
- **AND** it SHALL run the test command for the RunPod Endpoint Worker package
- **AND** it SHALL run Docker builds for both worker images using the shared worker Dockerfile
- **AND** it SHALL verify that both images contain the expected image-baked base runtime metadata, Python interpreter, ComfyUI root, base dependency records, manifest compatibility metadata, and overlay policy metadata
- **AND** it SHALL verify that a selected recipe for an existing runtime contract id/version matches that contract's compatibility metadata before publishing either image
- **AND** it SHALL continue to registry publication only after validation succeeds for the image pair

#### Scenario: Endpoint compatibility validation passes
- **WHEN** the runtime deployment workflow has built both worker images
- **THEN** it SHALL verify the image pair declares the same runtime contract id, version, implementation revision, image runtime compatibility metadata, and workspace overlay policy
- **AND** it SHALL continue only after pair compatibility validation succeeds

#### Scenario: Runtime contract compatibility validation fails
- **WHEN** the selected runtime recipe uses an existing runtime contract id/version but changes Python version, platform, ComfyUI revision, PyTorch index URL, PyTorch package list, base requirements, runtime manifest compatibility metadata, image runtime layout, or workspace overlay policy
- **THEN** the workflow SHALL fail before publishing either worker image
- **AND** the workflow SHALL report that the runtime contract version must be bumped or the recipe restored to the existing compatibility surface
- **AND** the workflow MUST NOT propose a Runtime Catalog update for the incompatible image pair

#### Scenario: Worker validation fails
- **WHEN** any required worker validation, image build validation, image runtime validation, image-pair compatibility step, or runtime contract compatibility step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag
- **AND** the workflow MUST NOT propose a Runtime Catalog update for the failed image pair

### Requirement: Select non-duplicate runtime implementation revisions
The runtime recipe release workflow SHALL resolve an implementation revision that does not already exist in the selected runtime contract before it builds or publishes worker images.

#### Scenario: Manual dispatch auto-selects fresh implementation revision
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** the workflow SHALL allow the implementation revision to be omitted
- **AND** when omitted or set to `auto`, the workflow SHALL select the next date-based implementation revision that does not already exist under the selected Runtime Catalog contract
- **AND** the workflow SHALL use that revision for provisioner image metadata, endpoint image metadata, deterministic image tags, and the generated Runtime Catalog update

#### Scenario: Manual dispatch accepts explicit fresh implementation revision
- **WHEN** an authorized operator starts the runtime deployment workflow manually with an explicit implementation revision
- **THEN** the workflow SHALL use that revision for provisioner image metadata, endpoint image metadata, deterministic image tags, and the generated Runtime Catalog update

#### Scenario: Duplicate implementation revision is rejected before publication
- **WHEN** the runtime deployment workflow resolves an implementation revision that already exists in the bundled Runtime Catalog entry for the selected recipe's runtime contract id and version
- **THEN** the workflow SHALL fail before worker package validation, Docker image builds, registry publication, and Runtime Catalog PR creation
- **AND** the workflow SHALL report that the implementation revision already exists

#### Scenario: Fresh implementation revision continues release
- **WHEN** the runtime deployment workflow resolves an implementation revision that does not already exist in the bundled Runtime Catalog entry for the selected recipe's runtime contract id and version
- **THEN** the workflow SHALL continue to worker validation, image build validation, image-pair compatibility validation, and runtime contract compatibility validation

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
- **WHEN** an operator needs to deploy or roll back worker images
- **THEN** documentation SHALL describe the runtime recipe workflow triggers
- **AND** documentation SHALL describe runtime recipe selection
- **AND** documentation SHALL describe automatic implementation revision selection and explicit override behavior
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced immutable image tags and digest-pinned refs
- **AND** documentation SHALL describe reviewed Runtime Catalog update PRs
- **AND** documentation SHALL describe rollback by selecting previously published immutable image pairs from Runtime Catalog entries

### Requirement: Build deterministic ComfyUI runtime in provisioner image
The worker Docker build SHALL construct the deterministic ComfyUI base runtime for the selected runtime recipe inside both provisioner and endpoint images before either image can be published.

#### Scenario: Runtime archive is built
- **WHEN** the provisioner and endpoint worker images are built for a runtime recipe
- **THEN** the Docker build SHALL install the fixed Python runtime, recipe-declared PyTorch/CUDA-compatible dependencies, ComfyUI, ComfyUI frontend/docs/templates, and ComfyUI base requirements into the image runtime root
- **AND** the Docker build SHALL produce metadata describing the runtime contract, implementation revision, included base runtime revisions, image runtime root, Python interpreter path, ComfyUI root path, base dependency records, manifest compatibility, and overlay policy
- **AND** the Docker build MUST NOT install Workflow Preset Custom Nodes or their Python dependencies into the image-baked base runtime
- **AND** base runtime dependency installation MUST happen during Docker build rather than container startup or workspace provisioning

#### Scenario: Runtime archive build fails
- **WHEN** the Docker build cannot install or verify any deterministic ComfyUI runtime dependency, image runtime metadata, or base dependency record
- **THEN** the Docker build SHALL fail
- **AND** no runtime recipe release workflow SHALL publish that image pair

