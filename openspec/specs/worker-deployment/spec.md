# worker-deployment Specification

## Purpose

Defines how LumaForge deploys worker container images from Git through repository automation.
## Requirements
### Requirement: Deploy Worker Images from Git

The repository SHALL provide separate GitHub Actions workflows that deploy worker container images from tracked Git source and the shared worker Dockerfile.

#### Scenario: Provisioner release tag deploys provisioner image
- **WHEN** a release tag matching the provisioner worker deployment trigger is pushed
- **THEN** the provisioner workflow SHALL validate the Provisioner Worker
- **AND** the provisioner workflow SHALL build the provisioner container image from the shared worker Dockerfile
- **AND** the provisioner workflow SHALL publish the provisioner image to GitHub Container Registry

#### Scenario: Endpoint release tag deploys endpoint image
- **WHEN** a release tag matching the RunPod endpoint worker deployment trigger is pushed
- **THEN** the endpoint workflow SHALL validate the RunPod Endpoint Worker
- **AND** the endpoint workflow SHALL build the endpoint container image from the shared worker Dockerfile
- **AND** the endpoint workflow SHALL publish the endpoint image to GitHub Container Registry

#### Scenario: Manual dispatch deploys one worker image
- **WHEN** an authorized operator starts one worker deployment workflow manually
- **THEN** that workflow SHALL validate only the selected worker
- **AND** that workflow SHALL build only the selected worker image from the selected Git ref and shared worker Dockerfile
- **AND** that workflow SHALL publish only the selected worker image to GitHub Container Registry using an immutable commit SHA tag

#### Scenario: Manual dispatch selects endpoint provider
- **WHEN** an authorized operator starts the endpoint worker deployment workflow manually
- **THEN** the workflow SHALL require an endpoint provider selection
- **AND** the workflow SHALL map the selected provider to that provider's worker package, shared Dockerfile target, and image name
- **AND** the workflow SHALL fail before publishing when the selected provider is not supported

### Requirement: Validate worker before publishing

Each worker deployment workflow SHALL complete that worker's validation successfully before publishing its image.

#### Scenario: Worker validation passes
- **WHEN** a workflow is preparing to publish a worker image
- **THEN** it SHALL run the test command for that worker package
- **AND** it SHALL run the Docker build for that worker image using the shared worker Dockerfile
- **AND** it SHALL continue to registry publication only after validation succeeds for that image

#### Scenario: Provisioner image smoke validation passes
- **WHEN** the provisioner worker deployment workflow has built the provisioner container image
- **THEN** it SHALL run the provisioner container smoke test against that built image before registry authentication or publication
- **AND** it SHALL verify the container starts, accepts authorized `GET /status`, returns `idle`, and contains its runtime Python dependencies
- **AND** it SHALL continue to registry publication only after the smoke test succeeds

#### Scenario: Worker validation fails
- **WHEN** any required worker validation step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag

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

The repository SHALL document how to operate the worker deployment workflows.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy or roll back worker images
- **THEN** documentation SHALL describe the workflow triggers
- **AND** documentation SHALL describe manual endpoint provider selection
- **AND** documentation SHALL describe the GitHub Container Registry image paths for the provisioner and endpoint images
- **AND** documentation SHALL describe produced image tags
- **AND** documentation SHALL describe rollback by selecting previously published immutable commit SHA tags for the affected worker images

