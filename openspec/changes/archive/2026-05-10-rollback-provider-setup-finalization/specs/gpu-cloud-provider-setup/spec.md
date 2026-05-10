## MODIFIED Requirements

### Requirement: Setup validates and stores the first Provider API Key

The Native Layer SHALL expose a command to create GPU Cloud Provider setup by validating a submitted Provider API Key and storing it only in the secure keyring when no complete setup already exists.

#### Scenario: New key is valid

- **WHEN** the Client submits `runpod` and a valid active Provider API Key
- **THEN** the Native Layer SHALL validate the key with RunPod identity before writing it
- **AND** the Native Layer SHALL store the key in the secure keyring
- **AND** the Native Layer SHALL re-read the stored key and return the live `GpuCloudProviderSetup`

#### Scenario: Existing setup is rejected

- **WHEN** a keyring entry already exists and the Client submits another Provider API Key for `runpod`
- **THEN** the Native Layer SHALL reject the request with `provider_setup_already_exists`
- **AND** the Native Layer SHALL reject the request before validating the submitted Provider API Key with RunPod
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Submitted key is empty

- **WHEN** the Client submits an empty Provider API Key
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Submitted key is invalid

- **WHEN** the Client submits a Provider API Key that RunPod rejects during identity validation
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Secure keyring write fails

- **WHEN** no previous key is stored and the submitted key validates successfully but secure keyring write fails
- **THEN** the Native Layer SHALL reject the request with `secure_keyring_unavailable`
- **AND** the Native Layer MUST NOT report setup success unless the key can be re-read and validated

#### Scenario: Stored key re-read fails after setup write and rollback succeeds

- **WHEN** no previous key is stored, the submitted key validates successfully, the key is written, and the Native Layer cannot re-read the stored key
- **AND** the Native Layer successfully deletes the newly written keyring entry
- **THEN** the Native Layer SHALL reject the request with `secure_keyring_unavailable`
- **AND** the Native Layer MUST NOT leave a provider keyring entry from the failed setup attempt
- **AND** a later setup retry MUST NOT be rejected because of the failed setup attempt

#### Scenario: Stored key validation fails after setup write and rollback succeeds

- **WHEN** no previous key is stored, the submitted key validates successfully, the key is written and re-read, and RunPod identity validation for the stored key fails
- **AND** the Native Layer successfully deletes the newly written keyring entry
- **THEN** the Native Layer SHALL reject the request with the stored-key validation error
- **AND** the Native Layer MUST NOT leave a provider keyring entry from the failed setup attempt
- **AND** a later setup retry MUST NOT be rejected because of the failed setup attempt

#### Scenario: Setup finalization fails and rollback fails

- **WHEN** no previous key is stored, the submitted key validates successfully, the key is written, setup finalization fails, and deleting the newly written keyring entry fails
- **THEN** the Native Layer SHALL reject the request with `provider_setup_recovery_required`
- **AND** the Native Layer MUST NOT report setup success
- **AND** the Native Layer MUST NOT expose the submitted or stored Provider API Key in the error response
