# gpu-cloud-provider-setup Specification

## Purpose
Define the native-owned GPU Cloud Provider setup lifecycle for validating, storing, reading, and deleting the local RunPod API key setup without exposing Provider API Keys to the Client.

## Requirements
### Requirement: Read live GPU Cloud Provider setup status

The Native Layer SHALL expose a command to read GPU Cloud Provider setup status for an explicit `gpu_cloud_provider_id`.

#### Scenario: Supported provider has no stored key

- **WHEN** the Client requests setup status for `runpod` and no local keyring entry exists
- **THEN** the Native Layer SHALL return `gpu_cloud_provider_setup: null`

#### Scenario: Supported provider has a stored active key

- **WHEN** the Client requests setup status for `runpod` and a local keyring entry exists
- **THEN** the Native Layer SHALL call RunPod identity using the stored key
- **AND** the Native Layer SHALL return a `GpuCloudProviderSetup` derived from the provider identity response

#### Scenario: Provider id is outside the generated command contract

- **WHEN** the Client uses generated command bindings
- **THEN** the Client SHALL only be able to submit supported `GpuCloudProviderId` values
- **AND** arbitrary unsupported provider ids are treated as command payload/schema violations outside the structured provider setup domain error contract

#### Scenario: Stored key cannot be validated due to network failure

- **WHEN** the Client requests setup status and RunPod identity is unavailable due to timeout, transport, or provider availability failure
- **THEN** the Native Layer SHALL reject the request with `provider_api_unavailable`

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

### Requirement: Serialize provider setup mutations

The Native Layer SHALL serialize GPU Cloud Provider setup mutations for the same `gpu_cloud_provider_id` so setup creation and deletion evaluate against the latest durable keyring state.

#### Scenario: Concurrent setup requests create at most one setup

- **WHEN** two setup requests for `runpod` run concurrently and no local keyring entry exists before either request starts
- **THEN** the Native Layer SHALL allow at most one request to store a Provider API Key and return setup success
- **AND** any later-observing setup request SHALL reject with `provider_setup_already_exists`
- **AND** the later-observing setup request SHALL reject before validating its submitted Provider API Key with RunPod

#### Scenario: Setup success is derived from stored state

- **WHEN** a setup request for `runpod` validates the submitted Provider API Key and stores it in the secure keyring
- **THEN** the Native Layer SHALL re-read the stored key
- **AND** the Native Layer SHALL derive the returned `GpuCloudProviderSetup` from the re-read stored key
- **AND** the Native Layer MUST NOT report setup success unless the stored key can be re-read and validated

#### Scenario: Setup waits for concurrent delete

- **WHEN** a setup request for `runpod` starts while a delete request for `runpod` is evaluating or mutating local setup state
- **THEN** the setup request SHALL evaluate setup existence only after the delete operation has finished
- **AND** the setup request SHALL proceed or reject based on the latest durable keyring state

#### Scenario: Delete waits for concurrent setup

- **WHEN** a delete request for `runpod` starts while a setup request for `runpod` is validating or mutating local setup state
- **THEN** the delete request SHALL evaluate setup existence only after the setup operation has finished
- **AND** the delete request SHALL delete or reject based on the latest durable keyring state

### Requirement: Derive redacted setup from provider identity

The Native Layer SHALL derive `GpuCloudProviderSetup` from provider identity and provider-reported API key identity without returning or persisting the Provider API Key outside the secure keyring.

#### Scenario: RunPod identity maps to exactly one active API key

- **WHEN** RunPod identity returns `myself.email` and `apiKeys` containing exactly one item where the submitted or stored secret starts with `apiKeys[].id`
- **AND** the matched item has `isActive == true`
- **THEN** the Native Layer SHALL return `gpu_cloud_provider_id: "runpod"`
- **AND** the Native Layer SHALL return `provider_user_email` from `myself.email`
- **AND** the Native Layer SHALL return `provider_api_key_fingerprint` from the matched `apiKeys[].id`

#### Scenario: Matched RunPod API key is inactive

- **WHEN** RunPod identity succeeds but the matched API key has `isActive == false`
- **THEN** the Native Layer SHALL reject the request with `invalid_provider_api_key`

#### Scenario: RunPod API key fingerprint cannot be derived

- **WHEN** RunPod identity succeeds but no API key matches the secret prefix or multiple API keys match the secret prefix
- **THEN** the Native Layer SHALL reject the request with `provider_identity_unavailable`

### Requirement: Keep Provider API Key secret from the Client

The Native Layer MUST NOT return, persist outside secure keyring, log, or include Provider API Keys in generated frontend types, command responses, errors, or diagnostics.

#### Scenario: Setup status is returned

- **WHEN** the Native Layer returns any successful GPU Cloud Provider setup response
- **THEN** the response SHALL include only redacted setup fields
- **AND** the response MUST NOT include the Provider API Key

#### Scenario: Setup command fails

- **WHEN** setup, status, or delete fails
- **THEN** the Native Layer error SHALL include a UI-safe code and message
- **AND** the error MUST NOT include the Provider API Key

### Requirement: Provider Setup services use domain-native inputs and results

Provider Setup services SHALL accept domain-native provider identifiers and secret value objects, and SHALL return domain setup snapshots instead of service-facing DTOs that duplicate command or domain models.

#### Scenario: Setup status is read

- **WHEN** a command requests GPU Cloud Provider setup status
- **THEN** the command boundary SHALL map the generated provider id DTO to a domain `GpuCloudProviderId`
- **AND** the Provider Setup service SHALL use the domain provider id directly
- **AND** the Provider Setup service SHALL return an optional domain `GpuCloudProviderSetup`
- **AND** the command boundary SHALL map the domain result into the generated command response DTO

#### Scenario: New setup is created

- **WHEN** a command submits a generated setup request containing a provider id and Provider API Key string
- **THEN** the command boundary or Provider Setup service SHALL convert the submitted key into the domain `ProviderApiKey` value object before provider validation
- **AND** the Provider Setup service SHALL return a domain `GpuCloudProviderSetup` after validating and storing the key
- **AND** neither the service result nor the generated command response MUST expose the Provider API Key

#### Scenario: Existing setup is deleted

- **WHEN** a command requests deletion for a GPU Cloud Provider setup
- **THEN** the command boundary SHALL map the generated provider id DTO to a domain `GpuCloudProviderId`
- **AND** the Provider Setup service SHALL use the domain provider id directly
- **AND** the Provider Setup service SHALL return a domain-native deletion result that does not require a service-facing response DTO
