## ADDED Requirements

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
