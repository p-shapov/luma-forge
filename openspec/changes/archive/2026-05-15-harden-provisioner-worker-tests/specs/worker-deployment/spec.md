## MODIFIED Requirements

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
