## ADDED Requirements

### Requirement: Select non-duplicate runtime implementation revisions
The runtime recipe release workflow SHALL resolve an implementation revision that does not already exist in the selected runtime contract before it builds or publishes worker images.

#### Scenario: Manual dispatch requires fresh implementation revision
- **WHEN** an authorized operator starts the runtime deployment workflow manually
- **THEN** the workflow SHALL require an implementation revision that is not pre-filled with a known existing Runtime Catalog implementation revision
- **AND** the workflow SHALL use that revision for provisioner image metadata, endpoint image metadata, deterministic image tags, and the generated Runtime Catalog update

#### Scenario: Duplicate implementation revision is rejected before publication
- **WHEN** the runtime deployment workflow resolves an implementation revision that already exists in the bundled Runtime Catalog entry for the selected recipe's runtime contract id and version
- **THEN** the workflow SHALL fail before worker package validation, Docker image builds, registry publication, and Runtime Catalog PR creation
- **AND** the workflow SHALL report that the implementation revision already exists

#### Scenario: Fresh implementation revision continues release
- **WHEN** the runtime deployment workflow resolves an implementation revision that does not already exist in the bundled Runtime Catalog entry for the selected recipe's runtime contract id and version
- **THEN** the workflow SHALL continue to worker validation, image build validation, image-pair compatibility validation, and runtime contract compatibility validation
