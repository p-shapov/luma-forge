## ADDED Requirements

### Requirement: Deploy Provisioner Worker from Git

The repository SHALL provide a GitHub Actions workflow that deploys the Provisioner Worker as a container image built from the tracked Git source.

#### Scenario: Release tag deploys worker image
- **WHEN** a release tag matching the worker deployment trigger is pushed
- **THEN** the workflow SHALL validate the Provisioner Worker
- **AND** the workflow SHALL build the container image from `workers/provisioner/Dockerfile`
- **AND** the workflow SHALL publish the image to GitHub Container Registry

#### Scenario: Manual dispatch deploys worker image
- **WHEN** an authorized operator starts the worker deployment workflow manually
- **THEN** the workflow SHALL validate the Provisioner Worker
- **AND** the workflow SHALL build the container image from the selected Git ref
- **AND** the workflow SHALL publish the image to GitHub Container Registry using an immutable commit SHA tag

### Requirement: Validate worker before publishing

The worker deployment workflow SHALL complete worker validation successfully before publishing any image.

#### Scenario: Worker validation passes
- **WHEN** the workflow is preparing to publish a worker image
- **THEN** it SHALL run the Provisioner Worker test command for `workers/provisioner`
- **AND** it SHALL run a Docker build for `workers/provisioner/Dockerfile`
- **AND** it SHALL continue to registry publication only after validation succeeds

#### Scenario: Worker validation fails
- **WHEN** any required worker validation step fails
- **THEN** the workflow SHALL fail the deployment
- **AND** the workflow MUST NOT publish or update any worker image tag

### Requirement: Tag published worker images deterministically

The worker deployment workflow SHALL publish deterministic image tags that identify the source Git revision.

#### Scenario: Commit image tag is published
- **WHEN** the workflow publishes a worker image
- **THEN** it SHALL publish an immutable tag containing the source commit SHA
- **AND** the image tag SHALL identify the exact Git commit used for the build

#### Scenario: Release image tag is published
- **WHEN** the workflow is triggered by a release tag
- **THEN** it SHALL publish a version tag derived from that Git tag
- **AND** it SHALL also publish the immutable commit SHA tag

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

The repository SHALL document how to operate the worker deployment workflow.

#### Scenario: Operator reads deployment documentation
- **WHEN** an operator needs to deploy or roll back a Provisioner Worker image
- **THEN** documentation SHALL describe the workflow triggers
- **AND** documentation SHALL describe the GitHub Container Registry image path
- **AND** documentation SHALL describe produced image tags
- **AND** documentation SHALL describe rollback by selecting a previously published immutable commit SHA tag
