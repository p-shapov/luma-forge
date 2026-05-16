## ADDED Requirements

### Requirement: Validate resolved runtime against provisioner image identity
The Provisioner Worker SHALL validate incoming resolved runtime implementation metadata against runtime identity provided by its running container configuration before materializing the workspace runtime.

#### Scenario: Provisioner image identity configuration is required
- **WHEN** the running Provisioner Worker configuration lacks `LUMA_FORGE_PROVISIONER_IMAGE_REF` or provides it as a blank value
- **THEN** the Provisioner Worker SHALL reject startup configuration with a UI-safe configuration error
- **AND** it MUST NOT fall back to a development placeholder image identity

#### Scenario: Matching runtime implementation is accepted
- **WHEN** `POST /start` contains a resolved runtime implementation whose contract id, contract version, implementation revision, and provisioner image ref match the running Provisioner Worker configuration
- **THEN** the Provisioner Worker SHALL continue runtime materialization
- **AND** it SHALL record the accepted runtime identity in the prepared runtime manifest after successful preparation

#### Scenario: Mismatched provisioner image ref is rejected
- **WHEN** `POST /start` contains a resolved runtime implementation whose provisioner image ref differs from the running Provisioner Worker configuration
- **THEN** the Provisioner Worker SHALL reject or fail the preparation job with a UI-safe runtime mismatch error
- **AND** it MUST NOT materialize the runtime archive or report terminal success

### Requirement: Publish image-baked base runtime records
The Provisioner Worker SHALL publish all image-baked base runtime dependency records declared by the resolved runtime metadata into their final workspace locations.

#### Scenario: Base runtime records are published
- **WHEN** runtime archive extraction succeeds
- **THEN** the Provisioner Worker SHALL publish the staged base runtime dependency records under the mounted workspace
- **AND** the prepared runtime manifest SHALL reference the published record files

#### Scenario: Missing declared base runtime record fails validation
- **WHEN** a declared base runtime dependency record is absent after archive extraction or publishing
- **THEN** the Provisioner Worker SHALL fail final validation
- **AND** it MUST NOT report terminal success
