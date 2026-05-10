## MODIFIED Requirements

### Requirement: Delete local GPU Cloud Provider setup

The Native Layer SHALL expose a command to delete the local GPU Cloud Provider setup for an explicit `gpu_cloud_provider_id`.

#### Scenario: Existing local setup is deleted

- **WHEN** the Client requests deletion for `runpod` and a provider keyring entry exists
- **THEN** the Native Layer SHALL delete the local keyring entry
- **AND** the Native Layer SHALL return `gpu_cloud_provider_setup: null`
- **AND** the Native Layer MUST NOT call RunPod or revoke the provider-side API key

#### Scenario: Corrupt local setup is deleted

- **WHEN** the Client requests deletion for `runpod` and a provider keyring entry exists but the stored value cannot be parsed as a valid Provider API Key
- **THEN** the Native Layer SHALL delete the local keyring entry
- **AND** the Native Layer SHALL return `gpu_cloud_provider_setup: null`
- **AND** the Native Layer MUST NOT call RunPod or revoke the provider-side API key

#### Scenario: Delete is requested when setup is missing

- **WHEN** the Client requests deletion for `runpod` and no provider keyring entry exists
- **THEN** the Native Layer SHALL reject the request with `provider_setup_incomplete`

#### Scenario: Delete keyring access fails

- **WHEN** the Client requests deletion and secure keyring entry lookup or deletion fails
- **THEN** the Native Layer SHALL reject the request with `secure_keyring_unavailable`
